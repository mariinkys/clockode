// SPDX-License-Identifier: GPL-3.0-only

use anywho::anywho;
use gstreamer::{
    self as gst,
    glib::{self, object::Cast},
    prelude::{ElementExt, GstBinExtManual},
};
use gstreamer_app as gst_app;
use iced::{
    Element,
    Length::{self},
    Subscription, Task,
    time::Instant,
    widget::{button, column, container, image, stack, text},
};
use smol::channel;
use std::{
    error::Error,
    fmt,
    os::fd::{AsRawFd, OwnedFd},
    sync::Arc,
};

use crate::{
    app::{
        utils::{InputableClockodeEntry, style},
        widgets::Toast,
    },
    icons,
};

pub struct QrScanPage {
    state: State,
}

enum State {
    AskingPermission,
    Permitted(Box<PermittedState>),
}

pub struct PermittedState {
    display_frame: Option<Box<image::Handle>>,
    pipeline: gst::Pipeline,
    frame_rx: channel::Receiver<FrameData>,
    display_rx: channel::Receiver<image::Handle>,
    _camera_fd: Option<Arc<OwnedFd>>, // we need to keep fd alive
}

impl Drop for QrScanPage {
    fn drop(&mut self) {
        if let State::Permitted(state) = &mut self.state {
            let _ = state.pipeline.set_state(gst::State::Null);
        }
    }
}

#[derive(Clone)]
struct FrameData {
    width: u32,
    height: u32,
    data: Vec<u8>,
}

#[derive(Debug, Clone)]
pub enum Message {
    /// Go back a screen
    Back,
    /// Callback after asking for camera permission
    PermissionCallback(Result<Arc<OwnedFd>, anywho::Error>),
    /// Callback after a QR is detected with the QR contents
    QrDetected(String),
    /// Updates the frame to be displayed
    UpdateDisplayFrame(Box<image::Handle>),
}

pub enum Action {
    /// Does nothing
    None,
    /// Go back a screen
    Back,
    /// Add a new [`Toast`] to show
    AddToast(Toast),
    /// Add a new [`Toast`] to show and goes back
    AddToastAndBack(Toast),
    /// Callback after an entry has been detected
    EntryDetected(InputableClockodeEntry),
}

#[derive(Debug)]
pub enum QrScanError {
    GStreamerInit(glib::Error),
    ElementCreation(&'static str),
    PipelineSetup(glib::BoolError),
    StateChange(gst::StateChangeError),
}

impl fmt::Display for QrScanError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            QrScanError::GStreamerInit(e) => write!(f, "Failed to initialize GStreamer: {}", e),
            QrScanError::ElementCreation(name) => write!(f, "Failed to create element: {}", name),
            QrScanError::PipelineSetup(e) => write!(f, "Failed to setup pipeline: {}", e),
            QrScanError::StateChange(e) => write!(f, "Failed to start pipeline: {}", e),
        }
    }
}

impl Error for QrScanError {}

impl QrScanPage {
    pub fn new() -> Result<(Self, Task<Message>), QrScanError> {
        gst::init().map_err(QrScanError::GStreamerInit)?;

        Ok((
            Self {
                state: State::AskingPermission,
            },
            Task::perform(
                async {
                    smol::future::or(
                        async { Self::request_camera_access().await.map(Arc::new) },
                        async {
                            smol::Timer::after(std::time::Duration::from_secs(30)).await;
                            Err(anywho!("Permission request timed out"))
                        },
                    )
                    .await
                },
                Message::PermissionCallback,
            ),
        ))
    }

    pub fn view(&self, _now: Instant) -> iced::Element<'_, Message> {
        let content = match &self.state {
            State::AskingPermission => container(text("Asking for camera permission..."))
                .center(Length::Fill)
                .into(),
            State::Permitted(state) => qr_scan_view(&state.display_frame),
        };

        container(content).padding(5.).center(Length::Fill).into()
    }

    pub fn update(&mut self, message: Message, _now: Instant) -> Action {
        match message {
            Message::Back => Action::Back,
            Message::PermissionCallback(res) => match res {
                Ok(fd) => match Self::init_gstreamer(fd) {
                    Ok(state) => {
                        self.state = state;
                        Action::None
                    }
                    Err(err) => Action::AddToastAndBack(Toast::error_toast(err)),
                },
                Err(err) => Action::AddToastAndBack(Toast::error_toast(err)),
            },
            Message::QrDetected(data) => match InputableClockodeEntry::try_from(data) {
                Ok(entry) => Action::EntryDetected(entry),
                Err(_) => Action::AddToast(Toast::warning_toast(
                    "QR Detected but it could not be decoded into an entry",
                )),
            },
            Message::UpdateDisplayFrame(handle) => {
                if let State::Permitted(state) = &mut self.state {
                    state.display_frame = Some(handle);
                }
                Action::None
            }
        }
    }

    pub fn subscription(&self, _now: Instant) -> Subscription<Message> {
        match &self.state {
            State::AskingPermission => Subscription::none(),
            State::Permitted(state) => Subscription::batch([
                Self::qr_detection(state.frame_rx.clone()),
                Self::update_camera_feed(state.display_rx.clone()),
            ]),
        }
    }

    /// Request camera access through the XDG Camera portal
    async fn request_camera_access() -> Result<OwnedFd, anywho::Error> {
        use ashpd::desktop::camera::Camera;

        let proxy = Camera::new().await?;
        proxy.request_access().await?;
        Ok(proxy.open_pipe_wire_remote().await?)
    }

    fn init_gstreamer(camera_fd: Arc<OwnedFd>) -> Result<State, QrScanError> {
        let pipeline = gst::Pipeline::new();
        let src = gst::ElementFactory::make("pipewiresrc")
            .property("fd", camera_fd.as_raw_fd())
            .build()
            .map_err(|_| QrScanError::ElementCreation("pipewiresrc"))?;
        let convert = gst::ElementFactory::make("videoconvert")
            .build()
            .map_err(|_| QrScanError::ElementCreation("videoconvert"))?;
        let sink = gst::ElementFactory::make("appsink")
            .build()
            .map_err(|_| QrScanError::ElementCreation("appsink"))?;

        pipeline
            .add_many([&src, &convert, &sink])
            .map_err(QrScanError::PipelineSetup)?;
        gst::Element::link_many([&src, &convert, &sink]).map_err(QrScanError::PipelineSetup)?;

        let appsink = sink
            .dynamic_cast::<gst_app::AppSink>()
            .map_err(|_| QrScanError::ElementCreation("appsink cast failed"))?;
        appsink.set_caps(Some(
            &gst::Caps::builder("video/x-raw")
                .field("format", "GRAY8")
                .field("width", 640i32)
                .field("height", 480i32)
                .build(),
        ));

        let (frame_tx, frame_rx) = channel::bounded::<FrameData>(1);
        let (display_tx, display_rx) = channel::bounded::<image::Handle>(1);

        appsink.set_callbacks(
            gst_app::AppSinkCallbacks::builder()
                .new_sample(move |sink| {
                    let sample = sink.pull_sample().map_err(|_| gst::FlowError::Error)?;
                    let buffer = sample.buffer().ok_or(gst::FlowError::Error)?;
                    let map = buffer.map_readable().map_err(|_| gst::FlowError::Error)?;
                    let caps = sample.caps().ok_or(gst::FlowError::Error)?;
                    let s = caps.structure(0).ok_or(gst::FlowError::Error)?;
                    let (w, h) = (
                        s.get::<i32>("width").map_err(|_| gst::FlowError::Error)? as u32,
                        s.get::<i32>("height").map_err(|_| gst::FlowError::Error)? as u32,
                    );

                    let data = map.to_vec();

                    // Update display frame
                    let mut rgba = Vec::with_capacity((w * h * 4) as usize);
                    for &gray in &data {
                        rgba.extend_from_slice(&[gray, gray, gray, 255]);
                    }
                    let handle = image::Handle::from_rgba(w, h, rgba);
                    let _ = display_tx.try_send(handle);

                    // Send grayscale data to QR decoder
                    let _ = frame_tx.try_send(FrameData {
                        width: w,
                        height: h,
                        data,
                    });

                    Ok(gst::FlowSuccess::Ok)
                })
                .build(),
        );

        pipeline
            .set_state(gst::State::Playing)
            .map_err(QrScanError::StateChange)?;

        Ok(State::Permitted(Box::new(PermittedState {
            display_frame: None,
            pipeline,
            frame_rx,
            display_rx,
            _camera_fd: Some(camera_fd),
        })))
    }

    fn update_camera_feed(display_rx: channel::Receiver<image::Handle>) -> Subscription<Message> {
        use iced::futures::sink::SinkExt;

        #[derive(Hash, Clone, Copy)]
        struct DisplayUpdaterId;

        #[derive(Clone)]
        struct DisplayData {
            id: DisplayUpdaterId,
            display_rx: channel::Receiver<image::Handle>,
        }

        impl std::hash::Hash for DisplayData {
            fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
                // We only hash the ID. We ignore the Receiver because
                // the identity of the subscription is tied to this ID.
                self.id.hash(state);
            }
        }

        let data = DisplayData {
            id: DisplayUpdaterId,
            display_rx,
        };

        Subscription::run_with(data, |data| {
            let display_rx = data.display_rx.clone();

            iced::stream::channel(
                10,
                move |mut output: iced::futures::channel::mpsc::Sender<Message>| async move {
                    while let Ok(handle) = display_rx.recv().await {
                        let _ = output
                            .send(Message::UpdateDisplayFrame(Box::new(handle)))
                            .await;
                    }

                    smol::future::pending::<()>().await;
                },
            )
        })
    }
}

/// QR Helper Functions
impl QrScanPage {
    fn qr_detection(frame_rx: channel::Receiver<FrameData>) -> Subscription<Message> {
        use iced::futures::sink::SinkExt;

        #[derive(Hash, Clone, Copy)]
        struct QrScannerId;

        #[derive(Clone)]
        struct ScannerData {
            id: QrScannerId,
            frame_rx: channel::Receiver<FrameData>,
        }

        impl std::hash::Hash for ScannerData {
            fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
                // We only hash the ID. We ignore the Receiver because
                // the identity of the subscription is tied to this ID.
                self.id.hash(state);
            }
        }

        let data = ScannerData {
            id: QrScannerId,
            frame_rx,
        };

        Subscription::run_with(data, |data| {
            let frame_rx = data.frame_rx.clone();

            iced::stream::channel(
                10,
                move |mut output: iced::futures::channel::mpsc::Sender<Message>| async move {
                    let mut last_check = std::time::Instant::now();
                    let interval = std::time::Duration::from_millis(300);

                    while let Ok(frame) = frame_rx.recv().await {
                        if last_check.elapsed() >= interval {
                            let frame_to_decode = frame.clone();

                            let res = smol::unblock(move || Self::decode_qr(frame_to_decode)).await;

                            if let Some(content) = res {
                                let _ = output.send(Message::QrDetected(content)).await;
                                last_check = std::time::Instant::now();
                            }
                        }
                    }

                    smol::future::pending::<()>().await;
                },
            )
        })
    }

    fn decode_qr(frame: FrameData) -> Option<String> {
        let mut img = rqrr::PreparedImage::prepare_from_greyscale(
            frame.width as usize,
            frame.height as usize,
            |x, y| frame.data[y * frame.width as usize + x],
        );

        img.detect_grids()
            .first()
            .and_then(|grid| grid.decode().ok())
            .map(|(_, content)| content)
    }
}

fn qr_scan_view<'a>(display_frame: &'a Option<Box<image::Handle>>) -> Element<'a, Message> {
    let camera_display = if let Some(handle) = display_frame {
        container(
            image(handle.as_ref().clone())
                .width(Length::Fill)
                .height(Length::Fill)
                .content_fit(iced::ContentFit::Contain),
        )
        .padding(40.)
        .center(Length::Fill)
    } else {
        container(
            column![
                icons::get_icon("camera-photo-symbolic", 48),
                text("Waiting for camera...").size(style::font_size::TITLE),
                text("Make sure camera permissions are granted")
                    .size(style::font_size::BODY)
                    .style(style::muted_text),
            ]
            .spacing(style::spacing::MEDIUM)
            .align_x(iced::Alignment::Center),
        )
        .center(Length::Fill)
    };

    let camera_with_button = container(stack![
        camera_display,
        container(
            button(icons::get_icon("go-previous-symbolic", 21))
                .on_press(Message::Back)
                .padding(8)
                .style(style::secondary_button)
        )
        .align_x(iced::alignment::Horizontal::Left)
        .align_y(iced::alignment::Vertical::Top),
        container(text("Point camera at QR code").size(style::font_size::TITLE))
            .width(Length::Fill)
            .align_x(iced::alignment::Horizontal::Center)
            .align_y(iced::alignment::Vertical::Top)
    ])
    .width(Length::Fill)
    .height(Length::Fill)
    .padding(10);

    column![camera_with_button]
        .spacing(style::spacing::MEDIUM)
        .padding(10)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}
