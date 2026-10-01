use super::*;

impl IntoNotedown for SmartLink {
    #[inline]
    fn into_node(self, span: &Span, file: &FileID) -> NotedownNode {
        NotedownKind::LinkNode(self).into_node(span, file)
    }
}

impl IntoNotedown for EmailLink {
    #[inline]
    fn into_node(self, span: &Span, file: &FileID) -> NotedownNode {
        SmartLink::EMail(Box::new(self)).into_node(span, file)
    }
}

impl IntoNotedown for HyperLink {
    #[inline]
    fn into_node(self, span: &Span, file: &FileID) -> NotedownNode {
        SmartLink::Normal(Box::new(self)).into_node(span, file)
    }
}

impl IntoNotedown for ImageLink {
    #[inline]
    fn into_node(self, span: &Span, file: &FileID) -> NotedownNode {
        SmartLink::Image(Box::new(self)).into_node(span, file)
    }
}

impl IntoNotedown for ListView {
    #[inline]
    fn into_node(self, span: &Span, file: &FileID) -> NotedownNode {
        NotedownKind::ListView(self).into_node(span, file)
    }
}

impl IntoNotedown for TableView {
    #[inline]
    fn into_node(self, span: &Span, file: &FileID) -> NotedownNode {
        NotedownKind::TableView(self).into_node(span, file)
    }
}

impl IntoNotedown for SimpleTable {
    #[inline]
    fn into_node(self, span: &Span, file: &FileID) -> NotedownNode {
        TableView::SimpleTable(Box::new(self)).into_node(span, file)
    }
}

impl NotedownKind {
    /// Constructor of [`SmartLink`]
    #[inline]
    pub fn smart_link(value: SmartLink, span: &Span, file: &FileID) -> NotedownNode {
        NotedownKind::LinkNode(value).into_node(span, file)
    }
    /// Getter of [`SmartLink`]
    pub fn get_smart_link(&self) -> Option<&SmartLink> {
        match self {
            NotedownKind::LinkNode(v) => Some(v),
            _ => None,
        }
    }
    /// Mutable getter of [`SmartLink`]
    pub fn mut_smart_link(&mut self) -> Option<&mut SmartLink> {
        match self {
            NotedownKind::LinkNode(v) => Some(v),
            _ => None,
        }
    }
    /// Constructor of [`EmailLink`]
    #[inline]
    pub fn email_link(value: EmailLink, span: &Span, file: &FileID) -> NotedownNode {
        SmartLink::EMail(Box::new(value)).into_node(span, file)
    }
    /// Getter of [`EmailLink`]
    pub fn get_email_link(&self) -> Option<&EmailLink> {
        match self.get_smart_link()? {
            SmartLink::EMail(v) => Some(v),
            _ => None,
        }
    }
    /// Mutable getter of [`EmailLink`]
    pub fn mut_email_link(&mut self) -> Option<&mut EmailLink> {
        match self.mut_smart_link()? {
            SmartLink::EMail(v) => Some(v),
            _ => None,
        }
    }
    /// Constructor of [`HyperLink`]
    #[inline]
    pub fn hyper_link(value: HyperLink, span: &Span, file: &FileID) -> NotedownNode {
        SmartLink::Normal(Box::new(value)).into_node(span, file)
    }
    /// Getter of [`HyperLink`]
    pub fn get_hyper_link(&self) -> Option<&HyperLink> {
        match self.get_smart_link()? {
            SmartLink::Normal(v) => Some(v),
            _ => None,
        }
    }
    /// Mutable getter of [`HyperLink`]
    pub fn mut_hyper_link(&mut self) -> Option<&mut HyperLink> {
        match self.mut_smart_link()? {
            SmartLink::Normal(v) => Some(v),
            _ => None,
        }
    }
    /// Constructor of [`ImageLink`]
    #[inline]
    pub fn image_link(value: ImageLink, span: &Span, file: &FileID) -> NotedownNode {
        SmartLink::Image(Box::new(value)).into_node(span, file)
    }
    /// Getter of [`ImageLink`]
    pub fn get_image_link(&self) -> Option<&ImageLink> {
        match self.get_smart_link()? {
            SmartLink::Image(v) => Some(v),
            _ => None,
        }
    }
    /// Mutable getter of [`ImageLink`]
    pub fn mut_image_link(&mut self) -> Option<&mut ImageLink> {
        match self.mut_smart_link()? {
            SmartLink::Image(v) => Some(v),
            _ => None,
        }
    }
    /// Constructor of [`ListView`]
    #[inline]
    pub fn list_view(value: ListView, span: &Span, file: &FileID) -> NotedownNode {
        NotedownKind::ListView(value).into_node(span, file)
    }
    /// Getter of [`ListView`]
    pub fn get_list_view(&self) -> Option<&ListView> {
        match self {
            NotedownKind::ListView(v) => Some(v),
            _ => None,
        }
    }
    /// Mutable getter of [`ListView`]
    pub fn mut_list_view(&mut self) -> Option<&mut ListView> {
        match self {
            NotedownKind::ListView(v) => Some(v),
            _ => None,
        }
    }
    /// Constructor of [`TableView`]
    #[inline]
    pub fn table_view(value: TableView, span: &Span, file: &FileID) -> NotedownNode {
        NotedownKind::TableView(value).into_node(span, file)
    }
    /// Getter of [`TableView`]
    pub fn get_table_view(&self) -> Option<&TableView> {
        match self {
            NotedownKind::TableView(v) => Some(v),
            _ => None,
        }
    }
    /// Mutable getter of [`TableView`]
    pub fn mut_table_view(&mut self) -> Option<&mut TableView> {
        match self {
            NotedownKind::TableView(v) => Some(v),
            _ => None,
        }
    }
    /// Constructor of [`SimpleTable`]
    #[inline]
    pub fn table_simple(value: SimpleTable, span: &Span, file: &FileID) -> NotedownNode {
        TableView::SimpleTable(Box::new(value)).into_node(span, file)
    }
    /// Getter of [`SimpleTable`]
    pub fn get_table_simple(&self) -> Option<&SimpleTable> {
        match self.get_table_view()? {
            TableView::SimpleTable(v) => Some(v),
            _ => None,
        }
    }
    /// Mutable getter of [`SimpleTable`]
    pub fn mut_table_simple(&mut self) -> Option<&mut SimpleTable> {
        match self.mut_table_view()? {
            TableView::SimpleTable(v) => Some(v),
            _ => None,
        }
    }
}
