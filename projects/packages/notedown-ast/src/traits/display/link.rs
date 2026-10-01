use super::*;
use crate::nodes::{EmailLink, HyperLink, ImageLink, ResourceDescriptor, TagReference, TwoWayLink};

impl Debug for SmartLink {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ExternalResource(v) => Debug::fmt(v, f),
            Self::Reference(v) => Debug::fmt(v, f),
            Self::Image(v) => Debug::fmt(v, f),
            Self::Normal(v) => Debug::fmt(v, f),
            Self::EMail(v) => Debug::fmt(v, f),
            Self::TwoWay(v) => Debug::fmt(v, f),
        }
    }
}

impl Display for SmartLink {
    fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
        match self {
            Self::Reference(v) => Display::fmt(v, f),
            Self::Image(v) => Display::fmt(v, f),
            Self::Normal(v) => Display::fmt(v, f),
            Self::EMail(v) => Display::fmt(v, f),
            Self::TwoWay(v) => Display::fmt(v, f),
            Self::ExternalResource(v) => Display::fmt(v, f),
        }
    }
}

impl Display for ResourceDescriptor {
    fn fmt(&self, _: &mut Formatter<'_>) -> std::fmt::Result {
        todo!()
    }
}

impl Display for EmailLink {
    fn fmt(&self, _: &mut Formatter<'_>) -> std::fmt::Result {
        todo!()
    }
}

impl Display for TagReference {
    fn fmt(&self, _: &mut Formatter<'_>) -> std::fmt::Result {
        todo!()
    }
}

impl Display for ImageLink {
    fn fmt(&self, _: &mut Formatter<'_>) -> std::fmt::Result {
        todo!()
    }
}

impl Debug for ImageLink {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let w = &mut f.debug_struct("ImageLink");
        w.field("src", &self.source);
        if let Some(s) = &self.description {
            w.field("alt", &s);
        }
        if let Some(s) = &self.size {
            w.field("width", &s.0);
            w.field("height", &s.1);
        }
        if let Some(s) = &self.layout {
            w.field("layout", &s);
        }
        w.finish()
    }
}

impl Debug for HyperLink {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let w = &mut f.debug_struct("HyperLink");
        w.field("src", &self.src);
        w.field("is_bare", &self.is_bare);
        if let Some(s) = &self.text {
            w.field("ast", &s);
        }
        if let Some(s) = &self.download {
            w.field("download", &s);
        }
        w.field("target", &self.target);
        w.finish()
    }
}

impl Display for HyperLink {
    // SmartLink::Path { path: from, to, alt, bind } => {
    //     let from_to = match to {
    //         None => format!("[{}]", from),
    //         Some(to) => format!("[{} > {}]", from, to),
    //     };
    //     match (bind, alt) {
    //         (None, None) => write!(f, "{}", from_to),
    //         (Some(bind), None) => write!(f, "{}[:{}]", from_to, bind),
    //         (None, Some(alt)) => write!(f, "{}[{}]", from_to, alt),
    //         (Some(bind), Some(alt)) => write!(f, "{}[{}:{}]", from_to, bind,
    // alt),     }
    // }
    // SmartLink::PathWithText { ast: img, path: to, alt, bind } => {
    //     let img_to = match to {
    //         None => format!("[{}]", img),
    //         Some(s) => format!("[{} > {}]", img, s),
    //     };
    //     match (bind, alt) {
    //         (None, None) => write!(f, "{}", img_to),
    //         (Some(bind), None) => write!(f, "{}[:{}]", img_to, bind),
    //         (None, Some(alt)) => write!(f, "{}[{}]", img_to, alt),
    //         (Some(bind), Some(alt)) => write!(f, "{}[{}:{}]", img_to, bind, alt),
    //     }
    // }
    fn fmt(&self, _: &mut Formatter<'_>) -> std::fmt::Result {
        todo!()
    }
}

impl Display for TwoWayLink {
    fn fmt(&self, _: &mut Formatter<'_>) -> std::fmt::Result {
        todo!()
    }
}
