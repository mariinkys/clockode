// SPDX-License-Identifier: GPL-3.0-only

use iced::advanced::layout::{self, Layout};
use iced::advanced::renderer::{self, Renderer as _};
use iced::advanced::widget::{self, Tree, Widget};
use iced::advanced::{Clipboard, Shell};
use iced::mouse;
use iced::{Border, Color, Element, Event, Length, Rectangle, Renderer, Size, Theme};

const DOT_SIZE: f32 = 12.0;

pub struct Dot {
    timer: u64,
}

impl Dot {
    pub fn new(timer: u64) -> Self {
        Self { timer }
    }

    fn color(&self, theme: &Theme) -> Color {
        let palette = theme.palette();

        if self.timer > 10 {
            palette.success
        } else if self.timer > 5 {
            palette.warning
        } else {
            palette.danger
        }
    }
}

impl<Message> Widget<Message, Theme, Renderer> for Dot {
    fn size(&self) -> Size<Length> {
        Size {
            width: Length::Fixed(DOT_SIZE),
            height: Length::Fixed(DOT_SIZE),
        }
    }

    fn layout(
        &mut self,
        _tree: &mut Tree,
        _renderer: &Renderer,
        _limits: &layout::Limits,
    ) -> layout::Node {
        layout::Node::new(Size::new(DOT_SIZE, DOT_SIZE))
    }

    fn draw(
        &self,
        _tree: &Tree,
        renderer: &mut Renderer,
        theme: &Theme,
        _style: &renderer::Style,
        layout: Layout<'_>,
        _cursor: mouse::Cursor,
        _viewport: &Rectangle,
    ) {
        let bounds = layout.bounds();
        let center = bounds.center();
        let radius = DOT_SIZE / 2.0;

        renderer.fill_quad(
            renderer::Quad {
                bounds: Rectangle {
                    x: center.x - radius,
                    y: center.y - radius,
                    width: DOT_SIZE,
                    height: DOT_SIZE,
                },
                border: Border {
                    radius: (radius).into(),
                    ..Default::default()
                },
                ..Default::default()
            },
            self.color(theme),
        );
    }

    fn tag(&self) -> widget::tree::Tag {
        struct Marker;
        widget::tree::Tag::of::<Marker>()
    }

    fn state(&self) -> widget::tree::State {
        widget::tree::State::None
    }

    fn update(
        &mut self,
        _tree: &mut Tree,
        _event: &Event,
        _layout: Layout<'_>,
        _cursor: mouse::Cursor,
        _renderer: &Renderer,
        _clipboard: &mut dyn Clipboard,
        _shell: &mut Shell<'_, Message>,
        _viewport: &Rectangle,
    ) {
        // No event handling needed for a static dot
    }

    fn mouse_interaction(
        &self,
        _tree: &Tree,
        _layout: Layout<'_>,
        _cursor: mouse::Cursor,
        _viewport: &Rectangle,
        _renderer: &Renderer,
    ) -> mouse::Interaction {
        mouse::Interaction::default()
    }
}

impl<'a, Message> From<Dot> for Element<'a, Message>
where
    Message: 'a,
{
    fn from(dot: Dot) -> Self {
        Element::new(dot)
    }
}

// Helper function for ease of use
pub fn dot(timer: u64) -> Dot {
    Dot::new(timer)
}
