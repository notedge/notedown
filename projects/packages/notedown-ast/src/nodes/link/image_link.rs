use super::*;
use diagnostic_quick::QError;

/// Image position relevant to the document
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub enum ImageLayout {
    /// Image left alignment
    Left,
    /// Image center alignment
    Center,
    /// Image right alignment
    Right,
}

/// ```note
/// [!! storage]
/// [./path/path.png : alt ast ]
/// [!description][source][link](options)
/// ```
#[derive(Clone, Default, Eq, PartialEq, Hash)]
pub struct ImageLink {
    ///
    pub source: String,
    /// alt in html
    pub description: Option<String>,
    /// alt in html
    pub link: Option<String>,
    /// force to use caption ignore global setting
    pub force_caption: Option<bool>,
    /// force to use layout ignore global setting
    /// -1: left
    /// 0: center
    /// 1: right
    pub layout: Option<ImageLayout>,
    /// force to use layout ignore global setting
    pub size: Option<(usize, usize)>,
    /// Additional options for the image
    pub options: Option<CommandArguments>,
}

impl ImageLink {
    /// Set the source of the image
    #[inline]
    pub fn set_src(&mut self, msg: impl Into<String>) {
        self.source = msg.into();
    }
    /// Set the description of the image
    #[inline]
    pub fn set_alt(&mut self, msg: impl Into<String>) {
        self.description = Some(msg.into());
    }
    /// Set the link of the image
    #[inline]
    pub fn set_link(&mut self, msg: impl Into<String>) {
        self.link = Some(msg.into());
    }
    /// Set the width and height of the image
    #[inline]
    pub fn set_size(&mut self, width: usize, height: usize) {
        self.size = Some((width, height));
    }
    /// Set the layout of the image
    #[inline]
    pub fn set_layout(&mut self, layout: ImageLayout) {
        self.layout = Some(layout);
    }
    /// Parse attributes from options
    pub fn parse_options(&mut self, options: CommandArguments) -> Vec<QError> {
        let mut args = options;
        let mut errors = vec![];

        args.optional.extract_string("src").map(|f| self.set_src(f));
        args.optional.extract_string("source").map(|f| self.set_src(f));

        self.parse_layout(&mut args, &mut errors);

        args.optional.extract_string("alt").map(|f| self.set_alt(f));
        args.optional.extract_string("caption").map(|f| self.set_alt(f));
        args.optional.extract_string("description").map(|f| self.set_alt(f));

        args.optional.extract_bool("force_caption").map(|f| self.force_caption = Some(f));
        if !args.is_empty() {
            self.options = Some(args);
        }
        return errors;
    }

    fn parse_layout(&mut self, options: &mut CommandArguments, errors: &mut Vec<QError>) {
        let value = match options.optional.extract("layout") {
            None => return,
            Some(s) => s,
        };
        match value {
            NotedownValue::Integer(i) if i.is_negative() => self.layout = Some(ImageLayout::Left),
            NotedownValue::Integer(i) if i.is_zero() => self.layout = Some(ImageLayout::Center),
            NotedownValue::Integer(i) if i.is_zero() => self.layout = Some(ImageLayout::Right),
            NotedownValue::String(s) if s.to_ascii_lowercase().eq("left") => self.layout = Some(ImageLayout::Left),
            NotedownValue::String(s) if s.to_ascii_lowercase().eq("center") => self.layout = Some(ImageLayout::Center),
            NotedownValue::String(s) if s.to_ascii_lowercase().eq("right") => self.layout = Some(ImageLayout::Right),
            _ => errors.push(QError::runtime_error(format!("Unknown layout option {}", value))),
        }
    }
}
