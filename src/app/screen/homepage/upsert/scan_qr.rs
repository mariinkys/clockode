// SPDX-License-Identifier: GPL-3.0-only

use gstreamer::{
    self as gst,
    glib::{
        self,
        object::{Cast, ObjectExt},
    },
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
    sync::{Arc, Mutex},
    time::Duration,
};

use crate::{
    app::{
        utils::{InputableClockodeEntry, style},
        widgets::Toast,
    },
    icons,
};

pub struct QrScanPage {
    display_frame: Arc<Mutex<Option<image::Handle>>>,
    pipeline: gst::Pipeline,
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
    /// Used to refresh iced view and show the camera feed
    Tick,
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
            .map_err(|_| QrScanError::ElementCreation("appsink (cast failed)"))?;
        appsink.set_property("drop", true);
        appsink.set_property("max-buffers", 1u32);
        appsink.set_caps(Some(
            &gst::Caps::builder("video/x-raw")
                .field("format", "RGBA")
                .build(),
        ));

        let display_frame = Arc::new(std::sync::Mutex::new(None));
        let display_frame_clone = display_frame.clone();

        let (frame_tx, frame_rx) = channel::bounded::<FrameData>(1);

        appsink.set_callbacks(
            gst_app::AppSinkCallbacks::builder()
                .new_sample(move |appsink| {
                    let sample = appsink.pull_sample().map_err(|_| gst::FlowError::Error)?;
                    let buffer = sample.buffer().ok_or(gst::FlowError::Error)?;
                    let map = buffer.map_readable().map_err(|_| gst::FlowError::Error)?;

                    let caps = sample.caps().ok_or(gst::FlowError::Error)?;
                    let s = caps.structure(0).ok_or(gst::FlowError::Error)?;
                    let width = s.get::<i32>("width").map_err(|_| gst::FlowError::Error)? as u32;
                    let height = s.get::<i32>("height").map_err(|_| gst::FlowError::Error)? as u32;

                    let data = map.as_slice().to_vec();

                    // Update display
                    if let Ok(mut frame) = display_frame_clone.lock() {
                        *frame = Some(image::Handle::from_rgba(width, height, data.clone()));
                    }

                    let frame_data = FrameData {
                        width,
                        height,
                        data,
                    };
                    // Send to QR processor
                    drop(frame_tx.try_send(frame_data));

                    Ok(gst::FlowSuccess::Ok)
                })
                .build(),
        );

        pipeline
            .set_state(gst::State::Playing)
            .map_err(QrScanError::StateChange)?;

        let qr_task = Task::perform(Self::qr_processing_loop(frame_rx), |result| {
            result.map(Message::QrDetected).unwrap_or(Message::Tick)
        });

        Ok((
            Self {
                display_frame,
                pipeline,
            },
            qr_task,
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
            Message::Tick => Action::None,
        }
    }

    pub fn subscription(&self, _now: Instant) -> Subscription<Message> {
        iced::time::every(Duration::from_millis(66)).map(|_| Message::Tick)
    }
}

/// QR Helper Functions
impl QrScanPage {
    async fn qr_processing_loop(frame_rx: channel::Receiver<FrameData>) -> Result<String, ()> {
        const INTERVAL: Duration = Duration::from_millis(300);
        let mut last_check = smol::Timer::after(Duration::ZERO).await;

        loop {
            match frame_rx.recv().await {
                Ok(frame) => {
                    // Check if enough time has passed
                    if last_check.elapsed() < INTERVAL {
                        continue; // Skip frame
                    }

                    last_check = smol::Timer::after(Duration::ZERO).await;

                    if let Some(qr_data) = smol::unblock(move || Self::decode_qr(frame)).await {
                        return Ok(qr_data);
                    }
                }
                Err(_) => return Err(()),
            }
        }
    }

    fn decode_qr(frame: FrameData) -> Option<String> {
        let scale = 2;
        let new_width = frame.width / scale;
        let new_height = frame.height / scale;

        // Convert to grayscale with downsampling
        let gray: Vec<u8> = (0..new_height * new_width)
            .map(|i| {
                let x = (i % new_width) * scale;
                let y = (i / new_width) * scale;
                let idx = ((y * frame.width + x) * 4) as usize;

                if idx + 2 < frame.data.len() {
                    ((frame.data[idx] as f32 * 0.299)
                        + (frame.data[idx + 1] as f32 * 0.587)
                        + (frame.data[idx + 2] as f32 * 0.114)) as u8
                } else {
                    0
                }
            })
            .collect();

        let mut img = rqrr::PreparedImage::prepare_from_greyscale(
            new_width as usize,
            new_height as usize,
            |x, y| gray[y * new_width as usize + x],
        );

        img.detect_grids()
            .first()
            .and_then(|grid| grid.decode().ok())
            .map(|(_, content)| String::from_utf8_lossy(content.as_bytes()).to_string())
    }
}

fn qr_scan_view<'a>(display_frame: &'a Arc<Mutex<Option<image::Handle>>>) -> Element<'a, Message> {
    let camera_display = if let Some(handle) = display_frame.lock().unwrap().clone() {
        container(
            image(handle)
                .width(Length::Fill)
                .content_fit(iced::ContentFit::Contain),
        )
        .width(Length::Fill)
        .height(Length::Fill)
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
        .width(Length::Fill)
        .height(Length::Fill)
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
        .align_y(iced::alignment::Vertical::Top)
    ])
    .width(Length::Fill)
    .height(Length::Fill)
    .style(style::entry_card)
    .padding(10);

    let status = container(text("Point camera at QR code").size(style::font_size::MEDIUM))
        .padding(16)
        .width(Length::Fill)
        .center_x(Length::Fill)
        .style(style::entry_card);

    column![camera_with_button, status,]
        .spacing(style::spacing::MEDIUM)
        .padding(10)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}
