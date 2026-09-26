// SPDX-License-Identifier: GPL-3.0-only

// Toast widget based on: https://github.com/iced-rs/iced/blob/master/examples/toast/src/main.rs
// I mainly added QOL and styling changes

use std::fmt;

use iced::advanced::layout::{self, Layout};
use iced::advanced::overlay;
use iced::advanced::renderer::{self, Renderer as _};
use iced::advanced::shell;
use iced::advanced::widget::{self, Operation, Tree};
use iced::advanced::{Shell, Widget};
use iced::mouse;
use iced::time::{self, Duration, Instant};
use iced::widget::{button, container, row, text};
use iced::window;
use iced::{
    Alignment, Center, Element, Event, Fill, Fit, Length, Point, Rectangle, Renderer, Size, Theme,
    Vector,
};

pub const DEFAULT_TIMEOUT: u64 = 5;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Status {
    #[default]
    Primary,
    Secondary,
    Success,
    Danger,
    Warning,
}

impl Status {
    pub const ALL: &'static [Self] = &[
        Self::Primary,
        Self::Secondary,
        Self::Success,
        Self::Danger,
        Self::Warning,
    ];
}

impl fmt::Display for Status {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Status::Primary => "Primary",
            Status::Secondary => "Secondary",
            Status::Success => "Success",
            Status::Danger => "Danger",
            Status::Warning => "Warning",
        }
        .fmt(f)
    }
}

#[derive(Debug, Clone, Default)]
pub struct Toast {
    pub title: String,
    pub body: String,
    pub status: Status,
}

impl Toast {
    /// Returns a [`Toast`] with 'Success' as the title and a [`Status::Success`] with the provided content as it's body
    pub fn success_toast<T>(body: T) -> Toast
    where
        T: ToString,
    {
        Toast {
            title: String::from("Success"),
            body: body.to_string(),
            status: Status::Success,
        }
    }

    /// Returns a [`Toast`] with 'Error' as the title and a [`Status::Danger`] with the provided content as it's body
    pub fn error_toast<T>(body: T) -> Toast
    where
        T: ToString,
    {
        Toast {
            title: String::from("Error"),
            body: body.to_string(),
            status: Status::Danger,
        }
    }

    /// Returns a [`Toast`] with 'Warning' as the title and a [`Status::Warning`] with the provided content as it's body
    pub fn warning_toast<T>(body: T) -> Toast
    where
        T: ToString,
    {
        Toast {
            title: String::from("Warning"),
            body: body.to_string(),
            status: Status::Warning,
        }
    }
}

pub struct Manager<'a, Message> {
    content: Element<'a, Message>,
    toasts: Vec<Element<'a, Message>>,
    timeout_secs: u64,
    on_close: Box<dyn Fn(usize) -> Message + 'a>,
}

impl<'a, Message> Manager<'a, Message>
where
    Message: 'a + Clone,
{
    pub fn new(
        content: impl Into<Element<'a, Message>>,
        toasts: &'a [Toast],
        on_close: impl Fn(usize) -> Message + 'a,
    ) -> Self {
        let toasts = toasts
            .iter()
            .enumerate()
            .map(|(index, toast)| {
                container(
                    row![
                        text(format!("{}:", toast.title.as_str()))
                            .width(Fit)
                            .font(iced::Font {
                                weight: iced::font::Weight::Bold,
                                ..Default::default()
                            }),
                        text(toast.body.as_str()).width(Fit),
                        button(" X ")
                            .style(|t, s| {
                                let mut style = match toast.status {
                                    Status::Primary => button::primary(t, s),
                                    Status::Secondary => button::secondary(t, s),
                                    Status::Success => button::success(t, s),
                                    Status::Danger => button::danger(t, s),
                                    Status::Warning => button::warning(t, s),
                                };

                                style.border.radius = iced::border::radius(8.);
                                style
                            })
                            .on_press((on_close)(index))
                            .padding(3),
                    ]
                    .align_y(Center)
                    .spacing(3),
                )
                .width(Fit)
                .padding(10)
                .style(|t| {
                    let mut style = match toast.status {
                        Status::Primary => container::primary(t),
                        Status::Secondary => container::secondary(t),
                        Status::Success => container::success(t),
                        Status::Danger => container::danger(t),
                        Status::Warning => container::warning(t),
                    };

                    style.border.radius = iced::border::radius(8.);
                    style
                })
                .into()
            })
            .collect();

        Self {
            content: content.into(),
            toasts,
            timeout_secs: DEFAULT_TIMEOUT,
            on_close: Box::new(on_close),
        }
    }

    pub fn timeout(self, seconds: u64) -> Self {
        Self {
            timeout_secs: seconds,
            ..self
        }
    }
}

/// Widget state: the expiry instants of each toast, plus the cache
/// `layout::flex::resolve` needs to lay the toasts out.
#[derive(Default)]
struct ManagerState {
    instants: Vec<Option<Instant>>,
    cache: layout::flex::Cache,
}

impl<Message> Widget<Message, Theme, Renderer> for Manager<'_, Message> {
    fn size(&self) -> Size<Length> {
        self.content.as_widget().size()
    }

    fn layout(&mut self, tree: &mut Tree, renderer: &Renderer, limits: &layout::Limits) {
        let content = &mut tree.children[0];

        self.content.as_widget_mut().layout(content, renderer, limits);
        content.translation = Vector::ZERO;

        tree.size = content.size;
    }

    fn tag(&self) -> widget::tree::Tag {
        widget::tree::Tag::of::<ManagerState>()
    }

    fn state(&self) -> widget::tree::State {
        widget::tree::State::new(ManagerState::default())
    }

    fn diff(&mut self, tree: &mut Tree) {
        let instants = &mut tree.state.downcast_mut::<ManagerState>().instants;

        // Invalidating removed instants to None allows us to remove
        // them here so that diffing for removed / new toast instants
        // is accurate
        instants.retain(Option::is_some);

        match (instants.len(), self.toasts.len()) {
            (old, new) if old > new => {
                instants.truncate(new);
            }
            (old, new) if old < new => {
                instants.extend(std::iter::repeat_n(Some(Instant::now()), new - old));
            }
            _ => {}
        }

        tree.diff_children(
            &mut std::iter::once(&mut self.content)
                .chain(&mut self.toasts)
                .collect::<Vec<_>>(),
        );
    }

    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout,
        viewport: &Rectangle,
        renderer: &Renderer,
        operation: &mut dyn Operation,
    ) {
        operation.container(None, layout.bounds(), viewport);
        operation.traverse(&mut |operation| {
            let (layout, tree) = layout.iter_mut(&mut tree.children).next().unwrap();

            self.content
                .as_widget_mut()
                .operate(tree, layout, viewport, renderer, operation);
        });
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        layout: Layout,
        cursor: mouse::Cursor,
        renderer: &Renderer,
        shell: &mut Shell<'_, Message>,
        viewport: &Rectangle,
    ) {
        let (layout, tree) = layout.iter_mut(&mut tree.children).next().unwrap();

        self.content
            .as_widget_mut()
            .update(tree, event, layout, cursor, renderer, shell, viewport);
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut Renderer,
        theme: &Theme,
        style: &renderer::Style,
        layout: Layout,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        let (layout, tree) = layout.iter(&tree.children).next().unwrap();

        self.content
            .as_widget()
            .draw(tree, renderer, theme, style, layout, cursor, viewport);
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
        renderer: &Renderer,
    ) -> mouse::Interaction {
        let (layout, tree) = layout.iter(&tree.children).next().unwrap();

        self.content
            .as_widget()
            .mouse_interaction(tree, layout, cursor, viewport, renderer)
    }

    fn overlay<'b>(
        &'b mut self,
        tree: &'b mut Tree,
        layout: Layout,
        renderer: &Renderer,
        viewport: &Rectangle,
        translation: Vector,
        window: Size,
    ) -> Vec<overlay::Element<'b, Message, Theme, Renderer>> {
        let state = tree.state.downcast_mut::<ManagerState>();
        let (content_tree, toast_trees) = tree.children.split_at_mut(1);

        let (content_layout, content_tree) = layout.iter_mut(content_tree).next().unwrap();

        let mut overlays = self.content.as_widget_mut().overlay(
            content_tree,
            content_layout,
            renderer,
            viewport,
            translation,
            window,
        );

        if self.toasts.is_empty() {
            return overlays;
        }

        // What used to be `Overlay::layout` now happens here
        let limits = layout::Limits::new(Size::ZERO, window).width(Fit.max(window.width * 0.75));

        let size = layout::flex::resolve(
            layout::flex::Axis::Vertical,
            renderer,
            &limits,
            Fit,
            Fill,
            10.into(),
            10.0,
            Alignment::Start,
            toast_trees,
            &mut self.toasts,
            &mut state.cache,
        );

        let position = layout.position() + translation;
        let x_offset = (window.width - size.width) / 2.0;

        overlays.push(overlay::Element::new(Box::new(Overlay {
            layout: Layout::new(size).move_to(Point::new(position.x + x_offset, position.y)),
            viewport: *viewport + translation,
            window,
            toasts: &mut self.toasts,
            trees: toast_trees,
            instants: &mut state.instants,
            on_close: &self.on_close,
            timeout_secs: self.timeout_secs,
        })));

        overlays
    }
}

struct Overlay<'a, 'b, Message> {
    layout: Layout,
    viewport: Rectangle,
    window: Size,
    toasts: &'b mut [Element<'a, Message>],
    trees: &'b mut [Tree],
    instants: &'b mut [Option<Instant>],
    on_close: &'b dyn Fn(usize) -> Message,
    timeout_secs: u64,
}

impl<Message> overlay::Overlay<Message, Theme, Renderer> for Overlay<'_, '_, Message> {
    fn update(
        &mut self,
        event: &Event,
        cursor: mouse::Cursor,
        renderer: &Renderer,
        shell: &mut Shell<'_, Message>,
    ) {
        if let Event::Window(window::Event::RedrawRequested(now)) = &event {
            self.instants
                .iter_mut()
                .enumerate()
                .for_each(|(index, maybe_instant)| {
                    if let Some(instant) = maybe_instant.as_mut() {
                        let remaining =
                            time::seconds(self.timeout_secs).saturating_sub(instant.elapsed());

                        if remaining == Duration::ZERO {
                            maybe_instant.take();
                            shell.publish((self.on_close)(index));
                        } else {
                            shell.request_redraw_at(*now + remaining);
                        }
                    }
                });
        }

        let viewport = self.layout.bounds();

        for ((child, (layout, tree)), instant) in self
            .toasts
            .iter_mut()
            .zip(self.layout.iter_mut(self.trees))
            .zip(self.instants.iter_mut())
        {
            let mut local_messages = shell::Bus::new();
            let mut local_shell = shell.local(&mut local_messages);

            child.as_widget_mut().update(
                tree,
                event,
                layout,
                cursor,
                renderer,
                &mut local_shell,
                &viewport,
            );

            if !local_shell.is_empty() {
                instant.take();
            }

            shell.merge(local_shell, std::convert::identity);
        }
    }

    fn draw(
        &self,
        renderer: &mut Renderer,
        theme: &Theme,
        style: &renderer::Style,
        cursor: mouse::Cursor,
    ) {
        let viewport = self.layout.bounds();

        renderer.with_layer(Rectangle::with_size(self.window), |renderer| {
            for (child, (layout, tree)) in self
                .toasts
                .iter()
                .zip(self.layout.iter(&self.trees[..]))
            {
                child
                    .as_widget()
                    .draw(tree, renderer, theme, style, layout, cursor, &viewport);
            }
        });
    }

    fn operate(&mut self, renderer: &Renderer, operation: &mut dyn widget::Operation) {
        operation.container(None, self.layout.bounds(), &self.viewport);
        operation.traverse(&mut |operation| {
            for (child, (layout, tree)) in self
                .toasts
                .iter_mut()
                .zip(self.layout.iter_mut(self.trees))
            {
                child
                    .as_widget_mut()
                    .operate(tree, layout, &self.viewport, renderer, operation);
            }
        });
    }

    fn mouse_interaction(&self, cursor: mouse::Cursor, renderer: &Renderer) -> mouse::Interaction {
        self.toasts
            .iter()
            .zip(self.layout.iter(&self.trees[..]))
            .map(|(child, (layout, tree))| {
                child
                    .as_widget()
                    .mouse_interaction(tree, layout, cursor, &self.viewport, renderer)
                    .max(if cursor.is_over(layout.bounds()) {
                        mouse::Interaction::Idle
                    } else {
                        Default::default()
                    })
            })
            .max()
            .unwrap_or_default()
    }
}

impl<'a, Message> From<Manager<'a, Message>> for Element<'a, Message>
where
    Message: 'a,
{
    fn from(manager: Manager<'a, Message>) -> Self {
        Element::new(manager)
    }
}
