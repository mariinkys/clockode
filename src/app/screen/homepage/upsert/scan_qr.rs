// SPDX-License-Identifier: GPL-3.0-only

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
use std::{error::Error, fmt};

use crate::{
    app::{
        utils::{InputableClockodeEntry, style},
        widgets::Toast,
    },
    icons,
};

pub struct QrScanPage {
    display_frame: Option<image::Handle>,
    pipeline: gst::Pipeline,
    frame_rx: channel::Receiver<FrameData>,
    display_rx: channel::Receiver<image::Handle>,
}

impl Drop for QrScanPage {
    fn drop(&mut self) {
        let _ = self.pipeline.set_state(gst::State::Null);
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
    /// Callback after a QR is detected with the QR contents
    QrDetected(String),
    /// Updates the frame to be displayed
    UpdateDisplayFrame(image::Handle),
}

pub enum Action {
    /// Does nothing
    None,
    /// Go back a screen
    Back,
    /// Add a new [`Toast`] to show
    AddToast(Toast),
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
        gstreamer::init().map_err(QrScanError::GStreamerInit)?;

        let pipeline = gst::Pipeline::new();
        let src = gst::ElementFactory::make("v4l2src")
            .build()
            .or_else(|_| gst::ElementFactory::make("autovideosrc").build())
            .map_err(|_| QrScanError::ElementCreation("video source"))?;
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

        Ok((
            Self {
                display_frame: None,
                pipeline,
                frame_rx,
                display_rx,
            },
            Task::none(),
        ))
    }

    pub fn view(&self, _now: Instant) -> iced::Element<'_, Message> {
        let content = qr_scan_view(&self.display_frame);

        container(content).padding(5.).center(Length::Fill).into()
    }

    pub fn update(&mut self, message: Message, _now: Instant) -> Action {
        match message {
            Message::Back => Action::Back,
            Message::QrDetected(data) => match InputableClockodeEntry::try_from(data) {
                Ok(entry) => Action::EntryDetected(entry),
                Err(_) => Action::AddToast(Toast::warning_toast(
                    "QR Detected but it could not be decoded into an entry",
                )),
            },
            Message::UpdateDisplayFrame(handle) => {
                self.display_frame = Some(handle);
                Action::None
            }
        }
    }

    pub fn subscription(&self, _now: Instant) -> Subscription<Message> {
        Subscription::batch([
            Self::qr_detection(self.frame_rx.clone()),
            Self::update_camera_feed(self.display_rx.clone()),
        ])
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
                        let _ = output.send(Message::UpdateDisplayFrame(handle)).await;
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

fn qr_scan_view<'a>(display_frame: &'a Option<image::Handle>) -> Element<'a, Message> {
    let camera_display = if let Some(handle) = display_frame {
        container(
            image(handle.clone())
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
