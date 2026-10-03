// SPDX-License-Identifier: GPL-3.0-only

use iced::advanced::layout::{self, Layout};
use iced::advanced::renderer::{self, Renderer as _};
use iced::advanced::widget::{self, Tree, Widget};
use iced::mouse;
use iced::{Border, Color, Length, Rectangle, Renderer, Size, Theme};

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
            palette.success.base.color
        } else if self.timer > 5 {
            palette.warning.base.color
        } else {
            palette.danger.base.color
        }
    }
}

impl widget::Meta for Dot {}

impl<Message> Widget<Message, Theme, Renderer> for Dot {
    fn size(&self) -> Size<Length> {
        Size {
            width: Length::Fixed(DOT_SIZE),
            height: Length::Fixed(DOT_SIZE),
        }
    }

    fn layout(&mut self, tree: &mut Tree, _renderer: &Renderer, _limits: &layout::Limits) {
        tree.size = Size::new(DOT_SIZE, DOT_SIZE);
    }

    fn draw(
        &self,
        _tree: &Tree,
        renderer: &mut Renderer,
        theme: &Theme,
        _style: &renderer::Style,
        layout: Layout,
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
                    radius: radius.into(),
                    ..Default::default()
                },
                ..Default::default()
            },
            self.color(theme),
        );
    }
}

pub fn dot(timer: u64) -> Dot {
    Dot::new(timer)
}
