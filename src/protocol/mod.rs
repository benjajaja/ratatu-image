//! Protocol backends for the widgets

use std::{
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
};

use image::{imageops, DynamicImage, ImageBuffer, Rgba};
use ratatui::{buffer::Buffer, layout::Rect};

use crate::FontSize;

use self::{
    halfblocks::{Halfblocks, StatefulHalfblocks},
    iterm2::{Iterm2, StatefulIterm2},
    kitty::{Kitty, StatefulKitty},
    sixel::{Sixel, StatefulSixel},
};

use super::Resize;

pub mod halfblocks;
pub mod iterm2;
pub mod kitty;
pub mod sixel;

trait ProtocolTrait: Send + Sync {
    /// Render the currently resized and encoded data to the buffer.
    fn render(&mut self, area: Rect, buf: &mut Buffer);
}

trait StatefulProtocolTrait: Send + Sync {
    // Get the background color that fills in when resizing.
    fn background_color(&self) -> Rgba<u8>;
    /// Check if the current image state would need resizing (grow or shrink) for the given area.
    ///
    /// This can be called by the UI thread to check if this [StatefulProtocol] should be sent off
    /// toprotoco
    /// some background thread/task to do the resizing and encoding, instead of rendering. The
    /// thread should then return the [StatefulProtocol] so that it can be rendered.protoco
    fn needs_resize(&mut self, resize: &Resize, area: Rect) -> Option<Rect>;

    /// Resize the image and encode it for rendering. The result should be stored statefully so
    /// that next call for the given area does not need to redo the work.
    ///
    /// This can be done in a background thread, and the result is stored in this [StatefulProtocol].
    fn resize_encode(&mut self, resize: &Resize, background_color: Rgba<u8>, area: Rect);

    /// Render the currently resized and encoded data to the buffer.
    fn render(&mut self, area: Rect, buf: &mut Buffer);
}

/// A fixed-size image protocol for the [crate::Image] widget.
#[derive(Clone)]
pub enum Protocol {
    Halfblocks(Halfblocks),
    Sixel(Sixel),
    Kitty(Kitty),
    ITerm2(Iterm2),
}
impl Protocol {
    pub(crate) fn render(&mut self, area: Rect, buf: &mut Buffer) {
        let inner: &mut dyn ProtocolTrait = match self {
            Self::Halfblocks(halfblocks) => halfblocks,
            Self::Sixel(sixel) => sixel,
            Self::Kitty(kitty) => kitty,
            Self::ITerm2(iterm2) => iterm2,
        };
        inner.render(area, buf);
    }
}

/// A stateful resizing image protocol for the [crate::StatefulImage] widget.
///
/// The [create::thread::ThreadImage] widget also uses this, and is the reason why resizing is
/// split from rendering.
#[derive(Clone)]
pub enum StatefulProtocol {
    Halfblocks(StatefulHalfblocks),
    Sixel(StatefulSixel),
    Kitty(StatefulKitty),
    ITerm2(StatefulIterm2),
}
impl StatefulProtocol {
    fn inner_trait(&self) -> &dyn StatefulProtocolTrait {
        match self {
            Self::Halfblocks(halfblocks) => halfblocks,
            Self::Sixel(sixel) => sixel,
            Self::Kitty(kitty) => kitty,
            Self::ITerm2(iterm2) => iterm2,
        }
    }
    fn inner_trait_mut(&mut self) -> &mut dyn StatefulProtocolTrait {
        match self {
            Self::Halfblocks(halfblocks) => halfblocks,
            Self::Sixel(sixel) => sixel,
            Self::Kitty(kitty) => kitty,
            Self::ITerm2(iterm2) => iterm2,
        }
    }

    pub fn background_color(&self) -> Rgba<u8> {
        let proto = self.inner_trait();
        proto.background_color()
    }

    /// Resize and encode if necessary, and render immediately.
    ///
    /// This blocks the UI thread but requires neither threads nor async.
    pub fn resize_encode_render(
        &mut self,
        resize: &Resize,
        background_color: Rgba<u8>,
        area: Rect,
        buf: &mut Buffer,
    ) {
        let proto = self.inner_trait_mut();
        if let Some(rect) = proto.needs_resize(resize, area) {
            proto.resize_encode(resize, background_color, rect);
        }
        proto.render(area, buf);
    }

    /// Check if the current image state would need resizing (grow or shrink) for the given area.
    ///
    /// This can be called by the UI thread to check if this [StatefulProtocol] should be sent off
    /// to some background thread/task to do the resizing and encoding, instead of rendering. The
    /// thread should then return the [StatefulProtocol] so that it can be rendered.protoco
    pub fn needs_resize(&mut self, resize: &Resize, area: Rect) -> Option<Rect> {
        match self {
            StatefulProtocol::Halfblocks(proto) => proto.needs_resize(resize, area),
            StatefulProtocol::Sixel(proto) => proto.needs_resize(resize, area),
            StatefulProtocol::Kitty(proto) => proto.needs_resize(resize, area),
            StatefulProtocol::ITerm2(proto) => proto.needs_resize(resize, area),
        }
    }

    /// Resize the image and encode it for rendering. The result should be stored statefully so
    /// that next call for the given area does not need to redo the work.
    ///
    /// This can be done in a background thread, and the result is stored in this [StatefulProtocol].
    pub fn resize_encode(&mut self, resize: &Resize, background_color: Rgba<u8>, area: Rect) {
        self.inner_trait_mut()
            .resize_encode(resize, background_color, area)
    }

    /// Render the currently resized and encoded data to the buffer.
    pub fn render(&mut self, area: Rect, buf: &mut Buffer) {
        self.inner_trait_mut().render(area, buf);
    }
}

#[derive(Clone)]
/// Image source for [crate::protocol::StatefulProtocol]s
///
/// A `[StatefulProtocol]` needs to resize the ImageSource to its state when the available area
/// changes. A `[Protocol]` only needs it once.
///
/// # Examples
/// ```text
/// use image::{DynamicImage, ImageBuffer, Rgb};
/// use ratatui_image::ImageSource;
///
/// let image: ImageBuffer::from_pixel(300, 200, Rgb::<u8>([255, 0, 0])).into();
/// let source = ImageSource::new(image, "filename.png", (7, 14));
/// assert_eq!((43, 14), (source.rect.width, source.rect.height));
/// ```
///
pub struct ImageSource {
    /// The original image without resizing.
    pub image: DynamicImage,
    /// The area that the [`ImageSource::image`] covers, but not necessarily fills.
    pub area: Rect,
    /// TODO: document this; when image changes but it doesn't need a resize, force a render.
    pub hash: u64,
    /// The background color that should be used for padding or background when resizing.
    pub background_color: Rgba<u8>,
}

impl ImageSource {
    /// Create a new image source
    pub fn new(
        mut image: DynamicImage,
        font_size: FontSize,
        background_color: Rgba<u8>,
    ) -> ImageSource {
        let area = ImageSource::round_pixel_size_to_cells(image.width(), image.height(), font_size);

        let mut state = DefaultHasher::new();
        image.as_bytes().hash(&mut state);
        let hash = state.finish();

        // We only need to underlay the background color here if it's not completely transparent.
        if background_color.0[3] != 0 {
            let mut bg: DynamicImage =
                ImageBuffer::from_pixel(image.width(), image.height(), background_color).into();
            imageops::overlay(&mut bg, &image, 0, 0);
            image = bg;
        }

        ImageSource {
            image,
            area,
            hash,
            background_color,
        }
    }
    /// Round an image pixel size to the nearest matching cell size, given a font size.
    fn round_pixel_size_to_cells(
        img_width: u32,
        img_height: u32,
        (char_width, char_height): FontSize,
    ) -> Rect {
        let width = (img_width as f32 / char_width as f32).ceil() as u16;
        let height = (img_height as f32 / char_height as f32).ceil() as u16;
        Rect::new(0, 0, width, height)
    }
}
