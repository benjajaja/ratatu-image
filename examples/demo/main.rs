#[cfg(all(
    not(feature = "crossterm"),
    not(feature = "termion"),
    not(feature = "termwiz")
))]
compile_error!("The demo needs one of the crossterm, termion, or termwiz features");

#[cfg(feature = "crossterm")]
mod crossterm;
#[cfg(feature = "termion")]
mod termion;
#[cfg(feature = "termwiz")]
mod termwiz;

use std::{env, error::Error, num::Wrapping as w, path::PathBuf, time::Duration};

use image::DynamicImage;
use ratatui::{
    backend::Backend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Style, Styled, Stylize},
    text::{Line, Text},
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame, Terminal,
};
use ratatui_image::{
    picker::Picker,
    protocol::{Protocol, StatefulProtocol},
    Image, Resize, StatefulImage,
};
use rustix::path::Arg;

fn main() -> Result<(), Box<dyn Error>> {
    #[cfg(feature = "crossterm")]
    crate::crossterm::run()?;
    #[cfg(feature = "termion")]
    crate::termion::run()?;
    #[cfg(feature = "termwiz")]
    crate::termwiz::run()?;
    Ok(())
}

#[derive(Debug)]
enum ShowImages {
    All,
    Fixed,
    Resized,
}

struct App {
    pub title: String,
    pub should_quit: bool,
    pub tick_rate: Duration,
    pub background: String,
    pub split_percent: u16,
    pub show_images: ShowImages,

    pub image_source_path: PathBuf,
    pub image_static_offset: (u16, u16),

    pub picker: Picker,
    pub image_source: DynamicImage,
    pub image_static: Protocol,
    pub image_fit_state: StatefulProtocol,
    pub image_crop_state: StatefulProtocol,
}

fn size() -> Rect {
    Rect::new(0, 0, 30, 16)
}

impl App {
    pub fn new<B: Backend>(_: &mut Terminal<B>) -> Self {
        let title = format!(
            "Demo ({})",
            env::var("TERM").unwrap_or("unknown".to_string())
        );

        let ada = "./assets/Ada.png";
        let image_source = image::io::Reader::open(ada).unwrap().decode().unwrap();

        let mut picker = Picker::from_query_stdio().unwrap();
        // Set completely transparent background (experimental, only works for iTerm2 and Kitty).
        picker.set_background_color([0, 0, 0, 0]);

        let image_static = picker
            .new_protocol(image_source.clone(), size(), Resize::Fit(None))
            .unwrap();

        let image_fit_state = picker.new_resize_protocol(image_source.clone());
        let image_crop_state = picker.new_resize_protocol(image_source.clone());

        let mut background = String::new();

        let mut r: [u64; 2] = [0x8a5cd789635d2dff, 0x121fd2155c472f96];
        for _ in 0..5_000 {
            let mut s1 = w(r[0]);
            let s0 = w(r[1]);
            let result = s0 + s1;
            r[0] = s0.0;
            s1 ^= s1 << 23;
            r[1] = (s1 ^ s0 ^ (s1 >> 18) ^ (s0 >> 5)).0;
            let c = match result.0 % 4 {
                0 => '.',
                1 => ' ',
                _ => '…',
            };
            background.push(c);
        }

        App {
            title,
            should_quit: false,
            tick_rate: Duration::from_millis(1000),
            background,
            show_images: ShowImages::All,
            split_percent: 70,
            picker,
            image_source,
            image_source_path: ada.into(),

            image_static,
            image_fit_state,
            image_crop_state,

            image_static_offset: (0, 0),
        }
    }
    pub fn on_key(&mut self, c: char) {
        match c {
            'q' => {
                self.should_quit = true;
            }
            't' => {
                self.show_images = match self.show_images {
                    ShowImages::All => ShowImages::Fixed,
                    ShowImages::Fixed => ShowImages::Resized,
                    ShowImages::Resized => ShowImages::All,
                }
            }
            'i' => {
                self.picker
                    .set_protocol_type(self.picker.protocol_type().next());
                self.reset_images();
            }
            'o' => {
                let path = match self.image_source_path.to_str() {
                    Some("./assets/Ada.png") => "./assets/Jenkins.jpg",
                    Some("./assets/Jenkins.jpg") => "./assets/NixOS.png",
                    _ => "./assets/Ada.png",
                };
                self.image_source = image::io::Reader::open(path).unwrap().decode().unwrap();
                self.image_source_path = path.into();
                self.reset_images();
            }
            'H' => {
                if self.split_percent >= 10 {
                    self.split_percent -= 10;
                }
            }
            'L' => {
                if self.split_percent <= 90 {
                    self.split_percent += 10;
                }
            }
            'h' => {
                if self.image_static_offset.0 > 0 {
                    self.image_static_offset.0 -= 1;
                }
            }
            'j' => {
                self.image_static_offset.1 += 1;
            }
            'k' => {
                if self.image_static_offset.1 > 0 {
                    self.image_static_offset.1 -= 1;
                }
            }
            'l' => {
                self.image_static_offset.0 += 1;
            }
            _ => {}
        }
    }

    fn reset_images(&mut self) {
        self.image_static = self
            .picker
            .new_protocol(self.image_source.clone(), size(), Resize::Fit(None))
            .unwrap();

        self.image_fit_state = self.picker.new_resize_protocol(self.image_source.clone());
        self.image_crop_state = self.picker.new_resize_protocol(self.image_source.clone());
    }

    pub fn on_tick(&mut self) {}
}

fn ui(f: &mut Frame<'_>, app: &mut App) {
    let outer_block = Block::default()
        .borders(Borders::TOP)
        .title(app.title.as_str());

    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints(
            [
                Constraint::Percentage(app.split_percent),
                Constraint::Percentage(100 - app.split_percent),
            ]
            .as_ref(),
        )
        .split(outer_block.inner(f.area()));
    f.render_widget(outer_block, f.area());

    let left_chunks = vertical_layout().split(chunks[0]);
    let right_chunks = vertical_layout().split(chunks[1]);

    let block_left_top = block("Fixed");
    let area = block_left_top.inner(left_chunks[0]);
    f.render_widget(paragraph(app.background.as_str().style(Color::Yellow), area));
    f.render_widget(block_left_top, left_chunks[0]);
    match app.show_images {
        ShowImages::Resized => {}
        _ => {
            let image = Image::new(&mut app.image_static);
            // Let it be surrounded by styled text.
            let offset_area = Rect {
                x: area.x + 1,
                y: area.y + 1,
                width: area.width.saturating_sub(2),
                height: area.height.saturating_sub(2),
            };
            f.render_widget(image, offset_area);
        }
    }

    let chunks_left_bottom = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)].as_ref())
        .split(left_chunks[1]);

    let block_left_bottom = block("Crop");
    let area = block_left_bottom.inner(chunks_left_bottom[0]);
    f.render_widget(
        paragraph(app.background.as_str()).style(Style::new().bg(Color::Green)),
        area,
    );
    match app.show_images {
        ShowImages::Fixed => {}
        _ => {
            let image = StatefulImage::default().resize(Resize::Crop(None));
            f.render_stateful_widget(
                image,
                block_left_bottom.inner(chunks_left_bottom[0]),
                &mut app.image_crop_state,
            );
        }
    }
    f.render_widget(block_left_bottom, chunks_left_bottom[0]);

    let block_middle_bottom = block("Placeholder");
    f.render_widget(
        paragraph(app.background.as_str()).style(Style::new().bg(Color::Blue)),
        block_middle_bottom.inner(chunks_left_bottom[1]),
    );
    f.render_widget(block_middle_bottom, chunks_left_bottom[1]);

<<<<<<< HEAD
    let block_right_top = Block::default()
        .borders(Borders::ALL)
        .border_style(Color::Blue)
        .title("Fit");
=======
    let block_right_top = block("Fit");
>>>>>>> f947fb4 (refactor(demo): simplify ui function which is required for updating the demo)
    let area = block_right_top.inner(right_chunks[0]);
    f.render_widget(paragraph(app.background.as_str()), area);
    match app.show_images {
        ShowImages::Fixed => {}
        _ => {
            let image = StatefulImage::default();
            f.render_stateful_widget(
                image,
                block_right_top.inner(right_chunks[0]),
                &mut app.image_fit_state,
            );
        }
    }
    f.render_widget(block_right_top, right_chunks[0]);

    let block_right_bottom = block("Help");
    let area = block_right_bottom.inner(right_chunks[1]);
    f.render_widget(
        paragraph(vec![
            Line::from("Key bindings:"),
            Line::from("H/L: resize"),
            Line::from(format!(
                "i: cycle image protocols (current: {:?})",
                app.picker.protocol_type()
            )),
            Line::from("o: cycle image"),
            Line::from(format!("t: toggle ({:?})", app.show_images)),
            Line::from(format!("Font size: {:?}", app.picker.font_size())),
        ]),
        area,
    );
    f.render_widget(block_right_bottom, right_chunks[1]);
}

fn paragraph<'a, T: Into<Text<'a>>>(str: T) -> Paragraph<'a> {
    Paragraph::new(str).wrap(Wrap { trim: true })
}

fn vertical_layout() -> Layout {
    Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)].as_ref())
}

fn block(name: &str) -> Block<'_> {
    Block::default().borders(Borders::ALL).title(name)
}
