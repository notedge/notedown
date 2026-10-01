use super::*;

pub mod hyper_link;
pub mod image_link;
pub mod other;
pub mod rd;
pub mod reference;
pub mod two_way;

/// 智能链接是指类似 `[ ]` 以及 `[[ ]]` 的结构
#[derive(Clone, Eq, PartialEq, Hash)]
pub enum SmartLink {
    /// - `[<RD>]`: Resource Descriptor
    ///
    /// ```note
    /// [./relative-path]: 使用相对路径
    /// [file://]: 使用绝对路径
    /// [https://]: 使用远程 url 路径
    /// [id/path]: 使用默认储存库
    /// [↯storage/id/path]: 使用具体的某个储存库
    /// ```
    ExternalResource(Box<ResourceDescriptor>),
    /// ```note
    /// [name↯example.com](options)
    /// ```
    EMail(Box<EmailLink>),
    /// ```note
    /// [<RD>](options)
    /// [describe][<RD>](options)
    /// ```
    Normal(Box<HyperLink>),
    /// ```note
    /// [<RD>](options)
    /// [!image-alt][<RD>](opts)
    /// ```
    Image(Box<ImageLink>),
    /// ## Tag Definition Block
    /// ```note
    /// [^tag-define]:
    ///     ast ast ast
    ///     ast ast ast
    /// ```
    /// ## Tag Reference Inline
    /// ```note
    /// [<RD>](opts)
    /// [^tag-name](options)
    /// [^tag-name][<RD>](opts)
    /// ```
    Reference(Box<TagReference>),
    /// ```note
    /// [[<RD>](options)]
    /// [[ast][<RD>](opts)]
    /// ```
    TwoWay(Box<TwoWayLink>),
}
