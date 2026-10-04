// SPDX-License-Identifier: GPL-3.0-only

//! A button that opens a dropdown of options when pressed.
//!
//! ```ignore
//! menu_button(
//!     row![icon, text("Import")],
//!     ImportType::ALL,
//!     ImportType::to_string,
//! )
//! .on_select(Message::OpenImportDialog)
//! .style(button::secondary)
//! ```
use iced::advanced::layout;
use iced::advanced::overlay;
use iced::advanced::renderer;
use iced::advanced::text::{self, Text, paragraph};
use iced::advanced::widget::Operation;
use iced::advanced::widget::tree::{self, Tree};
use iced::advanced::{Layout, Shell, Widget};
use iced::overlay::menu::{self, Menu};
use iced::widget::button::{self, Status, Style};
use iced::{
    Background, Border, Color, Event, Length, Padding, Pixels, Point, Rectangle, Size, Theme,
    Vector, alignment, mouse, touch, window,
};

/// Gap between the content and the arrow.
const ARROW_SPACING: f32 = 8.0;

/// Creates a new [`MenuButton`] with the given content and options.
pub fn menu_button<'a, T, Message, W>(
    content: W,
    options: impl Into<Vec<T>>,
    to_string: impl Fn(&T) -> String + 'a,
) -> MenuButton<'a, T, Message, W>
where
    T: Clone,
{
    MenuButton::new(content, options, to_string)
}

/// A button, with any content, that opens a list of options when pressed.
pub struct MenuButton<'a, T, Message, W> {
    content: W,
    options: Vec<T>,
    to_string: Box<dyn Fn(&T) -> String + 'a>,
    on_select: Option<Box<dyn Fn(T) -> Message + 'a>>,
    width: Length,
    height: Length,
    padding: Padding,
    text_size: Option<Pixels>,
    class: button::StyleFn<'a, Theme>,
    menu_class: menu::StyleFn<'a, Theme>,
    status: Option<Status>,
}

impl<'a, T, Message, W> MenuButton<'a, T, Message, W>
where
    T: Clone,
{
    /// Creates a new [`MenuButton`] with the given content and options.
    pub fn new(
        content: W,
        options: impl Into<Vec<T>>,
        to_string: impl Fn(&T) -> String + 'a,
    ) -> Self {
        Self {
            content,
            options: options.into(),
            to_string: Box::new(to_string),
            on_select: None,
            width: Length::Fit,
            height: Length::Fit,
            padding: button::DEFAULT_PADDING,
            text_size: None,
            class: Box::new(button::primary),
            menu_class: Box::new(|theme| menu_style(theme, &button::primary)),
            status: None,
        }
    }

    /// Sets the message produced when an option is chosen.
    ///
    /// Unless `on_select` is called, the [`MenuButton`] will be disabled.
    pub fn on_select(mut self, on_select: impl Fn(T) -> Message + 'a) -> Self {
        self.on_select = Some(Box::new(on_select));
        self
    }

    /// Sets the width of the [`MenuButton`].
    pub fn width(mut self, width: impl Into<Length>) -> Self {
        self.width = width.into();
        self
    }

    /// Sets the height of the [`MenuButton`].
    pub fn height(mut self, height: impl Into<Length>) -> Self {
        self.height = height.into();
        self
    }

    /// Sets the [`Padding`] of the [`MenuButton`] (also used for the options).
    pub fn padding(mut self, padding: impl Into<Padding>) -> Self {
        self.padding = padding.into();
        self
    }

    /// Sets the text size of the options.
    pub fn text_size(mut self, size: impl Into<Pixels>) -> Self {
        self.text_size = Some(size.into());
        self
    }

    /// Sets the style using any button style function (`button::secondary`,
    /// `button::danger`, your own...). The menu is restyled to match.
    #[must_use]
    pub fn style(mut self, style: impl Fn(&Theme, Status) -> Style + Clone + 'a) -> Self {
        let button = style.clone();

        self.class = Box::new(style);
        self.menu_class = Box::new(move |theme| menu_style(theme, &button));
        self
    }

    /// Overrides the style of the menu.
    #[must_use]
    pub fn menu_style(mut self, style: impl Fn(&Theme) -> menu::Style + 'a) -> Self {
        self.menu_class = Box::new(style);
        self
    }
}

/// Derives the menu look from the button look: a neutral surface with the
/// button's corner radius, and the hovered option painted like the hovered
/// button.
fn menu_style(theme: &Theme, button: &impl Fn(&Theme, Status) -> Style) -> menu::Style {
    let palette = theme.palette();
    let active = button(theme, Status::Active);
    let hovered = button(theme, Status::Hovered);

    menu::Style {
        background: palette.background.weak.color.into(),
        text_color: palette.background.weak.text,
        selected_background: hovered
            .background
            .unwrap_or(Background::Color(palette.background.strong.color)),
        selected_text_color: hovered.text_color,
        border: Border {
            radius: active.border.radius,
            width: 1.0,
            color: palette.background.strong.color,
        },
        ..menu::default(theme)
    }
}

struct State<P: text::Paragraph> {
    menu: menu::State,
    is_open: bool,
    hovered_option: Option<usize>,
    /// Measured option labels, so the menu can be wide enough for all of them.
    options: Vec<paragraph::Plain<P>>,
}

impl<'a, T, Message, W> iced::advanced::widget::Meta for MenuButton<'a, T, Message, W> {}

impl<'a, T, Message, W, Renderer> Widget<Message, Theme, Renderer> for MenuButton<'a, T, Message, W>
where
    T: Clone + 'a,
    Message: Clone + 'a,
    W: Widget<Message, Theme, Renderer>,
    Renderer: text::Renderer + 'a,
{
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<State<Renderer::Paragraph>>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(State::<Renderer::Paragraph> {
            menu: menu::State::default(),
            is_open: false,
            hovered_option: None,
            options: Vec::new(),
        })
    }

    fn diff(&mut self, tree: &mut Tree) {
        tree.diff_children(std::slice::from_mut(&mut self.content));

        let size = self.content.size();
        self.width = self.width.stack(size.width);
        self.height = self.height.stack(size.height);
    }

    fn size(&self) -> Size<Length> {
        Size {
            width: self.width,
            height: self.height,
        }
    }

    fn layout(&mut self, tree: &mut Tree, renderer: &Renderer, limits: &layout::Limits) {
        let text_size = self.text_size.unwrap_or_else(|| renderer.text_size());

        {
            let state = tree.state.downcast_mut::<State<Renderer::Paragraph>>();

            let line_height = renderer.line_height();

            state
                .options
                .resize_with(self.options.len(), Default::default);

            for (option, paragraph) in self.options.iter().zip(state.options.iter_mut()) {
                let label = (self.to_string)(option);

                let _ = paragraph.update(Text {
                    content: &label,
                    bounds: Size::new(f32::INFINITY, f32::from(line_height.to_absolute(text_size))),
                    size: text_size,
                    line_height,
                    font: renderer.font(),
                    align_x: text::Alignment::Default,
                    align_y: alignment::Vertical::Center,
                    shaping: text::Shaping::default(),
                    wrapping: text::Wrapping::None,
                    ellipsis: text::Ellipsis::None,
                    hint_factor: renderer.hint_factor(),
                });
            }
        }

        // Reserve room on the right for the arrow.
        let padding = Padding {
            right: self.padding.right + text_size.0 + ARROW_SPACING,
            ..self.padding
        };

        layout::padded(
            tree,
            limits,
            self.width,
            self.height,
            padding,
            |tree, limits| {
                self.content.layout(tree, renderer, limits);

                tree.size
            },
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
        let (content_layout, content_tree) = layout.iter_mut(&mut tree.children).next().unwrap();

        self.content.update(
            content_tree,
            event,
            content_layout,
            cursor,
            renderer,
            shell,
            viewport,
        );

        if shell.is_event_captured() {
            return;
        }

        let state = tree.state.downcast_mut::<State<Renderer::Paragraph>>();
        let is_hovered = cursor.is_over(layout.bounds());

        if let Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left))
        | Event::Touch(touch::Event::FingerPressed { .. }) = event
        {
            if state.is_open {
                // The overlay did not handle the press, so it landed outside
                // the menu (or on the button itself): close.
                state.is_open = false;
                shell.capture_event();
            } else if is_hovered && self.on_select.is_some() {
                state.is_open = true;
                state.hovered_option = None;
                shell.capture_event();
            }
        }

        let current_status = if self.on_select.is_none() {
            Status::Disabled
        } else if state.is_open {
            Status::Pressed
        } else if is_hovered {
            Status::Hovered
        } else {
            Status::Active
        };

        if let Event::Window(window::Event::RedrawRequested(_now)) = event {
            self.status = Some(current_status);
        } else if self.status.is_some_and(|status| status != current_status) {
            shell.request_redraw();
        }
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut Renderer,
        theme: &Theme,
        _style: &renderer::Style,
        layout: Layout,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        let bounds = layout.bounds();
        let (layout, tree) = layout.iter(&tree.children).next().unwrap();
        let style = (self.class)(theme, self.status.unwrap_or(Status::Disabled));

        if style.background.is_some() || style.border.width > 0.0 || style.shadow.color.a > 0.0 {
            renderer.fill_quad(
                renderer::Quad {
                    bounds,
                    border: style.border,
                    shadow: style.shadow,
                    snap: style.snap,
                },
                style
                    .background
                    .unwrap_or(Background::Color(Color::TRANSPARENT)),
            );
        }

        self.content.draw(
            tree,
            renderer,
            theme,
            &renderer::Style {
                text_color: style.text_color,
            },
            layout,
            cursor,
            viewport,
        );

        // The arrow, right-aligned inside the padding reserved in `layout`.
        let size = self.text_size.unwrap_or_else(|| renderer.text_size());
        let line_height = renderer.line_height();
        let hint_factor = renderer.hint_factor();

        renderer.fill_text(
            Text {
                content: Renderer::ARROW_DOWN_ICON.to_string(),
                bounds: Size::new(bounds.width, f32::from(line_height.to_absolute(size))),
                size,
                line_height,
                font: Renderer::ICON_FONT,
                align_x: text::Alignment::Right,
                align_y: alignment::Vertical::Center,
                shaping: text::Shaping::Basic,
                wrapping: text::Wrapping::None,
                ellipsis: text::Ellipsis::None,
                hint_factor,
            },
            Point::new(
                bounds.x + bounds.width - self.padding.right,
                bounds.center_y(),
            ),
            style.text_color,
            *viewport,
        );
    }

    fn mouse_interaction(
        &self,
        _tree: &Tree,
        layout: Layout,
        cursor: mouse::Cursor,
        _viewport: &Rectangle,
        _renderer: &Renderer,
    ) -> mouse::Interaction {
        if cursor.is_over(layout.bounds()) && self.on_select.is_some() {
            mouse::Interaction::Pointer
        } else {
            mouse::Interaction::default()
        }
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
        let is_open = tree
            .state
            .downcast_ref::<State<Renderer::Paragraph>>()
            .is_open;

        if !is_open {
            let (layout, tree) = layout.iter_mut(&mut tree.children).next().unwrap();

            return self
                .content
                .overlay(tree, layout, renderer, viewport, translation, window);
        }

        let Some(on_select) = &self.on_select else {
            return Vec::new();
        };

        let state = tree.state.downcast_mut::<State<Renderer::Paragraph>>();
        let bounds = layout.bounds();
        let position = layout.position() + translation;

        // At least as wide as the button, wider if an option needs it.
        let widest_option = state.options.iter().fold(0.0, |width, paragraph| {
            f32::max(width, paragraph.min_width())
        });
        let width = bounds.width.max(widest_option + self.padding.x());

        let mut menu = Menu::new(
            &mut state.menu,
            &self.options,
            &mut state.hovered_option,
            &self.to_string,
            |option| {
                state.is_open = false;

                (on_select)(option)
            },
            None,
            &self.menu_class,
        )
        .width(width)
        .padding(self.padding)
        .font(renderer.font());

        if let Some(text_size) = self.text_size {
            menu = menu.text_size(text_size);
        }

        vec![menu.overlay(renderer, position, window, bounds.height)]
    }
}
