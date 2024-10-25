//! Helper module to build a protocol, and swap protocols at runtime

use std::{
    env,
    io::{self, Read, Write},
    time::Duration,
};

use image::{DynamicImage, Rgb};
use ratatui::layout::Rect;
#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

use crate::{
    protocol::{
        halfblocks::{Halfblocks, StatefulHalfblocks},
        iterm2::{FixedIterm2, Iterm2State},
        kitty::{Kitty, StatefulKitty},
        sixel::{Sixel, StatefulSixel},
        Protocol, StatefulProtocol,
    },
    FontSize, ImageSource, Resize, Result,
};

#[derive(Clone, Copy, Debug)]
pub struct Picker {
    font_size: FontSize,
    protocol_type: ProtocolType,
    background_color: Option<Rgb<u8>>,
    is_tmux: bool,
    kitty_counter: u32,
}

/// Serde-friendly protocol-type enum for [Picker].
#[derive(PartialEq, Clone, Debug, Copy)]
#[cfg_attr(
    feature = "serde",
    derive(Deserialize, Serialize),
    serde(rename_all = "lowercase")
)]
pub enum ProtocolType {
    Halfblocks,
    Sixel,
    Kitty,
    Iterm2,
}

impl ProtocolType {
    pub fn next(&self) -> ProtocolType {
        match self {
            ProtocolType::Halfblocks => ProtocolType::Sixel,
            ProtocolType::Sixel => ProtocolType::Kitty,
            ProtocolType::Kitty => ProtocolType::Iterm2,
            ProtocolType::Iterm2 => ProtocolType::Halfblocks,
        }
    }
}

/// Helper for building widgets
impl Picker {
    /// Query terminal stdio for graphics capabilities and font-size with some escape sequences.
    ///
    /// This writes and reads from stdio momentarily. WARNING: this method should be called after
    /// entering alternate screen but before reading terminal events.
    ///
    /// # Example
    /// ```rust
    /// use ratatui_image::picker::Picker;
    /// let mut picker = Picker::from_query_stdio();
    /// ```
    ///
    pub fn from_query_stdio() -> Result<Picker> {
        // Detect tmux, and only if positive then take some risky guess for iTerm2 support.
        let (is_tmux, tmux_proto) = detect_tmux_and_outer_protocol_from_env();

        // Write and read to stdin to query protocol capabilities and font-size.
        let (capability_proto, font_size) = query_with_timeout(is_tmux, Duration::from_secs(1))?;

        // If some env var says that we should try iTerm2, then disregard protocol-from-capabilities.
        let iterm2_proto = iterm2_from_env();

        let protocol_type = tmux_proto
            .or(iterm2_proto)
            .or(capability_proto)
            .unwrap_or(ProtocolType::Halfblocks);

        if let Some(font_size) = font_size {
            Ok(Picker {
                font_size,
                background_color: None,
                protocol_type,
                is_tmux,
                kitty_counter: rand::random(),
            })
        } else {
            Err("could not query font size".into())
        }
    }

    /// Create a picker from a given terminal [FontSize].
    /// This is the only way to create a picker on windows, for now.
    ///
    /// # Example
    /// ```rust
    /// use ratatui_image::picker::Picker;
    ///
    /// let user_fontsize = (7, 14);
    ///
    /// let mut picker = Picker::from_fontsize(user_fontsize);
    /// ```
    pub fn from_fontsize(font_size: FontSize) -> Picker {
        // Detect tmux, and if positive then take some risky guess for iTerm2 support.
        let (is_tmux, tmux_proto) = detect_tmux_and_outer_protocol_from_env();

        // Disregard protocol-from-capabilities if some env var says that we could try iTerm2.
        let iterm2_proto = iterm2_from_env();

        let protocol_type = tmux_proto
            .or(iterm2_proto)
            .unwrap_or(ProtocolType::Halfblocks);

        Picker {
            font_size,
            background_color: None,
            protocol_type,
            is_tmux,
            kitty_counter: rand::random(),
        }
    }

    pub fn protocol_type(self) -> ProtocolType {
        self.protocol_type
    }

    pub fn set_protocol_type(&mut self, protocol_type: ProtocolType) {
        self.protocol_type = protocol_type;
    }

    pub fn font_size(self) -> FontSize {
        self.font_size
    }

    pub fn set_background_color(&mut self, background_color: Option<Rgb<u8>>) {
        self.background_color = background_color
    }

    /// Returns a new protocol for [`crate::Image`] widgets that fits into the given size.
    pub fn new_protocol(
        &mut self,
        image: DynamicImage,
        size: Rect,
        resize: Resize,
    ) -> Result<Box<dyn Protocol>> {
        let source = ImageSource::new(image, self.font_size);
        match self.protocol_type {
            ProtocolType::Halfblocks => Ok(Box::new(Halfblocks::from_source(
                &source,
                self.font_size,
                resize,
                self.background_color,
                size,
            )?)),
            ProtocolType::Sixel => Ok(Box::new(Sixel::from_source(
                &source,
                self.font_size,
                resize,
                self.background_color,
                self.is_tmux,
                size,
            )?)),
            ProtocolType::Kitty => {
                self.kitty_counter = self.kitty_counter.checked_add(1).unwrap_or(1);
                Ok(Box::new(Kitty::from_source(
                    &source,
                    self.font_size,
                    resize,
                    self.background_color,
                    size,
                    self.kitty_counter,
                    self.is_tmux,
                )?))
            }
            ProtocolType::Iterm2 => Ok(Box::new(FixedIterm2::from_source(
                &source,
                self.font_size,
                resize,
                self.background_color,
                self.is_tmux,
                size,
            )?)),
        }
    }

    /// Returns a new *stateful* protocol for [`crate::StatefulImage`] widgets.
    pub fn new_resize_protocol(&mut self, image: DynamicImage) -> Box<dyn StatefulProtocol> {
        let source = ImageSource::new(image, self.font_size);
        match self.protocol_type {
            ProtocolType::Halfblocks => Box::new(StatefulHalfblocks::new(source, self.font_size)),
            ProtocolType::Sixel => {
                Box::new(StatefulSixel::new(source, self.font_size, self.is_tmux))
            }
            ProtocolType::Kitty => {
                self.kitty_counter = self.kitty_counter.checked_add(1).unwrap_or(1);
                Box::new(StatefulKitty::new(
                    source,
                    self.font_size,
                    self.kitty_counter,
                    self.is_tmux,
                ))
            }
            ProtocolType::Iterm2 => {
                Box::new(Iterm2State::new(source, self.font_size, self.is_tmux))
            }
        }
    }
}

fn detect_tmux_and_outer_protocol_from_env() -> (bool, Option<ProtocolType>) {
    // Check if we're inside tmux.
    if !env::var("TERM").is_ok_and(|term| term.starts_with("tmux"))
        && !env::var("TERM_PROGRAM").is_ok_and(|term_program| term_program == "tmux")
    {
        return (false, None);
    }

    let _ = std::process::Command::new("tmux")
        .args(["set", "-p", "allow-passthrough", "on"])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .and_then(|mut child| child.wait()); // wait(), for check_device_attrs.

    // Crude guess based on the *existence* of some magic program specific env vars.
    // Produces false positives, for example xterm started from kitty inherits KITTY_WINDOW_ID.
    // Furthermore, tmux shares env vars from the first session, for example tmux started in xterm
    // after a previous tmux session started in kitty, inherits KITTY_WINDOW_ID.
    const OUTER_TERM_HINTS: [(&str, ProtocolType); 3] = [
        ("KITTY_WINDOW_ID", ProtocolType::Kitty), // TODO: query should work inside tmux, remove?
        ("ITERM_SESSION_ID", ProtocolType::Iterm2),
        ("WEZTERM_EXECUTABLE", ProtocolType::Iterm2),
    ];
    for (hint, proto) in OUTER_TERM_HINTS {
        if env::var(hint).is_ok_and(|s| !s.is_empty()) {
            return (true, Some(proto));
        }
    }
    (true, None)
}

fn iterm2_from_env() -> Option<ProtocolType> {
    if env::var("TERM_PROGRAM").is_ok_and(|term_program| {
        term_program.contains("iTerm")
            || term_program.contains("WezTerm")
            || term_program.contains("mintty")
            || term_program.contains("vscode")
            || term_program.contains("Tabby")
            || term_program.contains("Hyper")
    }) {
        return Some(ProtocolType::Iterm2);
    }
    if env::var("LC_TERMINAL").is_ok_and(|lc_term| lc_term.contains("iTerm")) {
        return Some(ProtocolType::Iterm2);
    }
    None
}

#[cfg(not(windows))]
fn enable_raw_mode() -> Result<impl FnOnce() -> Result<()>> {
    use rustix::termios::{self, LocalModes, OptionalActions};

    let stdin = io::stdin();
    let mut termios = termios::tcgetattr(&stdin)?;
    let termios_original = termios.clone();

    // Disable canonical mode to read without waiting for Enter, disable echoing.
    termios.local_modes &= !LocalModes::ICANON;
    termios.local_modes &= !LocalModes::ECHO;
    termios::tcsetattr(&stdin, OptionalActions::Drain, &termios)?;

    Ok(move || {
        Ok(termios::tcsetattr(
            io::stdin(),
            OptionalActions::Now,
            &termios_original,
        )?)
    })
}

#[cfg(not(windows))]
fn font_size_fallback() -> Option<FontSize> {
    use rustix::termios::{self, Winsize};

    let winsize = termios::tcgetwinsize(io::stdout()).ok()?;
    let Winsize {
        ws_xpixel: x,
        ws_ypixel: y,
        ws_col: cols,
        ws_row: rows,
    } = winsize;
    if x == 0 || y == 0 || cols == 0 || rows == 0 {
        return None;
    }

    Some((x / cols, y / rows))
}

#[cfg(windows)]
use windows::Win32::Foundation::HANDLE;

#[cfg(windows)]
fn current_in_handle() -> Result<HANDLE> {
    use windows::{
        core::PCWSTR,
        Win32::{
            Foundation::{GENERIC_READ, GENERIC_WRITE},
            Storage::FileSystem::{
                self, FILE_FLAGS_AND_ATTRIBUTES, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING,
            },
        },
    };

    let utf16: Vec<u16> = "CONIN$\0".encode_utf16().collect();
    let utf16_ptr: *const u16 = utf16.as_ptr();

    Ok(unsafe {
        FileSystem::CreateFileW(
            PCWSTR(utf16_ptr),
            (GENERIC_READ | GENERIC_WRITE).0,
            FILE_SHARE_READ | FILE_SHARE_WRITE,
            None,
            OPEN_EXISTING,
            FILE_FLAGS_AND_ATTRIBUTES(0),
            HANDLE::default(),
        )
    }?)
}

#[cfg(windows)]
fn enable_raw_mode() -> Result<impl FnOnce() -> Result<()>> {
    use windows::Win32::System::Console::{
        self, CONSOLE_MODE, ENABLE_ECHO_INPUT, ENABLE_LINE_INPUT, ENABLE_PROCESSED_INPUT,
    };

    let in_handle = current_in_handle()?;

    let mut original_in_mode = CONSOLE_MODE::default();
    unsafe { Console::GetConsoleMode(in_handle, &mut original_in_mode) }?;

    let requested_in_modes = !ENABLE_ECHO_INPUT & !ENABLE_LINE_INPUT & !ENABLE_PROCESSED_INPUT;
    let in_mode = original_in_mode & requested_in_modes;
    unsafe { Console::SetConsoleMode(in_handle, in_mode) }?;

    Ok(move || {
        let in_handle = current_in_handle()?;

        unsafe { Console::SetConsoleMode(in_handle, *original_in_mode) }?;

        Ok(())
    })
}

#[cfg(windows)]
fn font_size_fallback() -> Option<FontSize> {
    None
}

fn query_stdio_capabilities(is_tmux: bool) -> Result<(Option<ProtocolType>, Option<FontSize>)> {
    let (start, escape, end) = if is_tmux {
        ("\x1bPtmux;", "\x1b\x1b", "\x1b\\")
    } else {
        ("", "\x1b", "")
    };

    // Send several control sequences at once:
    // `_Gi=...`: Kitty graphics support.
    // `[c`: Capabilities including sixels.
    // `[16t`: Cell-size (perhaps we should also do `[14t`).
    // `[1337n`: iTerm2 (some terminals implement the protocol but sadly not this custom CSI)
    // `[5n`: Device Status Report, implemented by all terminals, ensure that there is some
    // response and we don't hang reading forever.
    let query = format!("{start}{escape}_Gi=31,s=1,v=1,a=q,t=d,f=24;AAAA{escape}\\{escape}[c{escape}[16t{escape}[1337n{escape}[5n{end}");
    io::stdout().write_all(query.as_bytes())?;
    io::stdout().flush()?;

    let mut parser = Parser::new();
    let mut capabilities = vec![];
    'out: loop {
        let mut charbuf: [u8; 50] = [0; 50];
        let result = io::stdin().read(&mut charbuf);
        match result {
            Ok(read) => {
                for ch in charbuf.iter().take(read) {
                    if let Some(cap) = parser.push(char::from(*ch)) {
                        if cap == ParsedResponse::Status {
                            break 'out;
                        } else {
                            capabilities.push(cap);
                        }
                    }
                }
            }
            Err(err) => {
                return Err(err.into());
            }
        }
    }

    if capabilities.is_empty() {
        return Err("no reply to graphics support query".into());
    }

    let mut proto = None;
    let mut font_size = None;
    if capabilities.contains(&ParsedResponse::Kitty(true)) {
        proto = Some(ProtocolType::Kitty);
    } else if capabilities.contains(&ParsedResponse::Sixel(true)) {
        proto = Some(ProtocolType::Sixel);
    }

    for cap in capabilities {
        if let ParsedResponse::CellSize(Some((w, h))) = cap {
            font_size = Some((w, h));
        }
    }
    // In case some terminal didn't support the cell-size query.
    font_size = font_size.or_else(font_size_fallback);

    Ok((proto, font_size))
}

struct Parser {
    data: String,
    sequence: ParsedResponse,
}

#[derive(Debug, PartialEq)]
enum ParsedResponse {
    Unknown,
    Kitty(bool),
    Sixel(bool),
    CellSize(Option<(u16, u16)>),
    Status,
}

impl Parser {
    pub fn new() -> Self {
        Parser {
            data: String::new(),
            sequence: ParsedResponse::Unknown,
        }
    }
    pub fn push(&mut self, next: char) -> Option<ParsedResponse> {
        match self.sequence {
            ParsedResponse::Unknown => {
                match (&self.data[..], next) {
                    (_, '\x1b') => {
                        // If the current sequence hasn't been identified yet, start a new one on Esc.
                        return self.restart();
                    }
                    ("[", '?') => {
                        self.sequence = ParsedResponse::Sixel(false);
                    }
                    ("_Gi=31", ';') => {
                        self.sequence = ParsedResponse::Kitty(false);
                    }
                    ("[6", ';') => {
                        self.sequence = ParsedResponse::CellSize(None);
                    }
                    ("[", '0') => {
                        self.sequence = ParsedResponse::Status;
                    }
                    _ => {}
                };
                self.data.push(next);
            }
            ParsedResponse::Sixel(_) => match next {
                'c' => {
                    // This is just easier than actually parsing the string.
                    let is_sixel = self.data.contains(";4;")
                        || self.data.contains("?4;")
                        || self.data.ends_with(";4")
                        || self.data.ends_with("?4");
                    self.data = String::new();
                    self.sequence = ParsedResponse::Unknown;
                    return Some(ParsedResponse::Sixel(is_sixel));
                }
                '\x1b' => {
                    return self.restart();
                }
                _ => {
                    self.data.push(next);
                }
            },

            ParsedResponse::Kitty(_) => match next {
                '\\' => {
                    let is_kitty = self.data == "_Gi=31;OK\x1b";
                    self.data = String::new();
                    self.sequence = ParsedResponse::Unknown;
                    return Some(ParsedResponse::Kitty(is_kitty));
                }
                _ => {
                    self.data.push(next);
                }
            },

            ParsedResponse::CellSize(_) => match next {
                't' => {
                    let mut cell_size = None;
                    let inner: Vec<&str> = self.data.split(';').collect();
                    if let [_, h, w] = inner[..] {
                        if let (Ok(h), Ok(w)) = (h.parse::<u16>(), w.parse::<u16>()) {
                            if w > 0 && h > 0 {
                                cell_size = Some((w, h));
                            }
                        }
                    }
                    self.data = String::new();
                    self.sequence = ParsedResponse::Unknown;
                    return Some(ParsedResponse::CellSize(cell_size));
                }
                '\x1b' => {
                    return self.restart();
                }
                _ => {
                    self.data.push(next);
                }
            },
            ParsedResponse::Status => match next {
                'n' => return Some(ParsedResponse::Status),
                '\x1b' => {
                    return self.restart();
                }
                _ => {
                    self.data.push(next);
                }
            },
        };
        None
    }
    fn restart(&mut self) -> Option<ParsedResponse> {
        self.data = String::new();
        self.sequence = ParsedResponse::Unknown;
        None
    }
}

fn query_with_timeout(
    is_tmux: bool,
    timeout: Duration,
) -> Result<(Option<ProtocolType>, Option<FontSize>)> {
    use std::{sync::mpsc, thread};
    let (tx, rx) = mpsc::channel();

    thread::spawn(move || {
        let _ = tx.send(
            enable_raw_mode()
                .and_then(|disable_raw_mode| {
                    let result = query_stdio_capabilities(is_tmux);
                    // Always try to return to raw_mode.
                    disable_raw_mode()?;
                    result
                })
                .map_err(|dyn_err| io::Error::new(io::ErrorKind::Other, dyn_err.to_string())),
        );
    });

    match rx.recv_timeout(timeout) {
        Ok(result) => Ok(result?),
        Err(err) => Err(err.into()),
    }
}

#[cfg(test)]
mod tests {
    use std::assert_eq;

    use crate::picker::{ParsedResponse, Parser, Picker, ProtocolType};

    #[test]
    fn test_cycle_protocol() {
        let mut proto = ProtocolType::Halfblocks;
        proto = proto.next();
        assert_eq!(proto, ProtocolType::Sixel);
        proto = proto.next();
        assert_eq!(proto, ProtocolType::Kitty);
        proto = proto.next();
        assert_eq!(proto, ProtocolType::Iterm2);
        proto = proto.next();
        assert_eq!(proto, ProtocolType::Halfblocks);
    }

    #[test]
    fn test_from_query_stdio_no_hang() {
        let _ = Picker::from_query_stdio();
    }

    #[test]
    fn test_parse_all() {
        for (name, str, expected) in vec![
            (
                "all",
                "\x1b_Gi=31;OK\x1b\\\x1b[?64;4c\x1b[6;7;14t\x1b[0n",
                vec![
                    ParsedResponse::Kitty(true),
                    ParsedResponse::Sixel(true),
                    ParsedResponse::CellSize(Some((14, 7))),
                    ParsedResponse::Status,
                ],
            ),
            ("only garbage", "\x1bhonkey\x1btonkey\x1b[42\x1b\\", vec![]),
            (
                "preceding garbage",
                "\x1bgarbage...\x1b[?64;5c\x1b[0n",
                vec![ParsedResponse::Sixel(false), ParsedResponse::Status],
            ),
            (
                "inner garbage",
                "\x1b[6;7;14t\x1bgarbage...\x1b[?64;5c\x1b[0n",
                vec![
                    ParsedResponse::CellSize(Some((14, 7))),
                    ParsedResponse::Sixel(false),
                    ParsedResponse::Status,
                ],
            ),
        ] {
            let mut parser = Parser::new();
            let mut caps = vec![];
            for ch in str.chars() {
                if let Some(cap) = parser.push(ch) {
                    caps.push(cap);
                }
            }
            assert_eq!(caps, expected, "{name}");
        }
    }
}
