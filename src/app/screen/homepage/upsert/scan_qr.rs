// SPDX-License-Identifier: GPL-3.0-only

use iced::{
    Element,
    Length::{self},
    Subscription, Task, event,
    keyboard::{self, Key, key::Named},
    time::Instant,
    widget::{button, column, container, image, stack, text},
};
use nokhwa::{
    Camera,
    pixel_format::LumaFormat,
    utils::{CameraIndex, RequestedFormat, RequestedFormatType, Resolution},
};
use smol::channel;
use std::sync::Arc;

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
    Permitted(Box<PermittedState>),
}

pub struct PermittedState {
    display_frame: Option<Box<image::Handle>>,
    frame_rx: channel::Receiver<FrameData>,
    display_rx: channel::Receiver<image::Handle>,
}

impl Drop for QrScanPage {
    fn drop(&mut self) {
        // nokhwa Camera is dropped automatically when PermittedState is dropped;
        // the capture thread will exit when its channel senders are dropped.
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
    /// Callback after pressing a [`Hotkey`] of this page
    Hotkey(Hotkey),
    /// Callback after opening the camera
    CameraReady(Result<(), Arc<nokhwa::NokhwaError>>),
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

impl QrScanPage {
    pub fn new() -> (Self, Task<Message>) {
        let (frame_tx, frame_rx) = channel::bounded::<FrameData>(1);
        let (display_tx, display_rx) = channel::bounded::<image::Handle>(1);

        let task = Task::perform(
            async move {
                smol::unblock(move || Self::open_camera_and_capture(frame_tx, display_tx))
                    .await
                    .map_err(Arc::new)
            },
            Message::CameraReady,
        );

        (
            Self {
                state: State::Permitted(Box::new(PermittedState {
                    display_frame: None,
                    frame_rx,
                    display_rx,
                })),
            },
            task,
        )
    }

    pub fn view(&self, _now: Instant) -> iced::Element<'_, Message> {
        let content = match &self.state {
            State::Permitted(state) => qr_scan_view(&state.display_frame),
        };

        container(content).padding(5.).center(Length::Fill).into()
    }

    pub fn update(&mut self, message: Message, _now: Instant) -> Action {
        match message {
            Message::Back => Action::Back,
            Message::Hotkey(hotkey) => match hotkey {
                Hotkey::Esc => Action::Back,
            },
            Message::CameraReady(res) => match res {
                Ok(()) => Action::None,
                Err(err) => Action::AddToastAndBack(Toast::error_toast(err)),
            },
            Message::QrDetected(data) => match InputableClockodeEntry::try_from(data) {
                Ok(entry) => Action::EntryDetected(entry),
                Err(_) => Action::AddToast(Toast::warning_toast(
                    "QR Detected but it could not be decoded into an entry",
                )),
            },
            Message::UpdateDisplayFrame(handle) => {
                let State::Permitted(state) = &mut self.state; {
                    state.display_frame = Some(handle);
                }
                Action::None
            }
        }
    }

    pub fn subscription(&self, _now: Instant) -> Subscription<Message> {
        match &self.state {
            State::Permitted(state) => Subscription::batch([
                Self::qr_detection(state.frame_rx.clone()),
                Self::update_camera_feed(state.display_rx.clone()),
                event::listen_with(handle_event),
            ]),
        }
    }

    /// Opens the camera and spawns a blocking capture loop.
    /// Sends grayscale frames to `frame_tx` and RGBA display handles to `display_tx`.
    /// Returns when the channel receivers are dropped (i.e. the page is gone).
    fn open_camera_and_capture(
        frame_tx: channel::Sender<FrameData>,
        display_tx: channel::Sender<image::Handle>,
    ) -> Result<(), nokhwa::NokhwaError> {
        let format = RequestedFormat::new::<LumaFormat>(RequestedFormatType::AbsoluteHighestResolution);
        let mut camera = Camera::new(CameraIndex::Index(0), format)?;

        // Prefer 640×480 if the camera supports it; nokhwa will pick the
        // closest match when using AbsoluteHighestResolution, so we can also
        // set an explicit resolution here.
        let _ = camera.set_resolution(Resolution::new(640, 480));
        camera.open_stream()?;

        loop {
            let frame = match camera.frame() {
                Ok(f) => f,
                Err(_) => continue,
            };

            let resolution = frame.resolution();
            let w = resolution.width();
            let h = resolution.height();

            // Decode the frame as luma (grayscale) bytes.
            let gray_bytes: Vec<u8> = match frame.decode_image::<LumaFormat>() {
                Ok(img) => img.into_raw(),
                Err(_) => continue,
            };

            // Build RGBA for the display channel.
            let mut rgba = Vec::with_capacity((w * h * 4) as usize);
            for &g in &gray_bytes {
                rgba.extend_from_slice(&[g, g, g, 255]);
            }
            let handle = image::Handle::from_rgba(w, h, rgba);

            // Non-blocking sends: if a receiver is full we just drop the frame
            // rather than block the capture loop.
            let _ = display_tx.try_send(handle);
            let _ = frame_tx.try_send(FrameData {
                width: w,
                height: h,
                data: gray_bytes,
            });

            // Exit cleanly once the page has been dropped.
            if display_tx.is_closed() || frame_tx.is_closed() {
                break;
            }
        }

        camera.stop_stream()?;
        Ok(())
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

//
// SUBSCRIPTIONS
//

#[derive(Debug, Clone)]
pub enum Hotkey {
    Esc,
}

fn handle_event(event: event::Event, _: event::Status, _: iced::window::Id) -> Option<Message> {
    #[allow(clippy::collapsible_match)]
    match event {
        event::Event::Keyboard(keyboard::Event::KeyPressed {
            key, modifiers: _, ..
        }) => match key {
            Key::Named(Named::Escape) => Some(Message::Hotkey(Hotkey::Esc)),
            _ => None,
        },
        _ => None,
    }
}
