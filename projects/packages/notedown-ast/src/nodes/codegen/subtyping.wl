(* ::Package:: *)

subtyping = <|
	(* link *)
    "SmartLink"-> <|
        "parent"-> "NotedownKind",
        "variant"-> "LinkNode",
        "box"-> False,
        "constructor"-> "smart_link"
    |>,
    "EmailLink"-> <|
        "parent"-> "SmartLink",
        "variant"-> "EMail",
        "box"-> True,
        "constructor"-> "email_link"
    |>,
    "HyperLink"-> <|
        "parent"-> "SmartLink",
        "variant"-> "Normal",
        "box"-> True,
        "constructor"-> "hyper_link"
    |>,
    "ImageLink"-> <|
        "parent"-> "SmartLink",
        "variant"-> "Image",
        "box"-> True,
        "constructor"-> "image_link"
    |>,
	(* list *)
    "ListView"-> <|
        "parent"-> "NotedownKind",
        "variant"-> "ListView",
        "box"-> False,
        "constructor"-> "list_view"
    |>,
    "OrderedList"-> <|
        "parent"-> "ListView",
        "variant"-> "Ordered",
        "box"-> True,
        "constructor"-> "list_ordered"
    |>,
    "OrderlessList"-> <|
        "parent"-> "ListView",
        "variant"-> "Orderless",
        "box"-> True,
        "constructor"-> "list_orderless"
    |>,
    (* quote *)
    "QuoteBlock"-> <|
        "parent"-> "TableView",
        "variant"-> "QuoteNode",
        "box"-> True,
        "constructor"-> "quote_block"
    |>
	(* table *)
    "TableView"-> <|
        "parent"-> "NotedownKind",
        "variant"-> "TableView",
        "box"-> False,
        "constructor"-> "table_view"
    |>,
    "SimpleTable"-> <|
        "parent"-> "TableView",
        "variant"-> "SimpleTable",
        "box"-> True,
        "constructor"-> "table_simple"
    |>
|>;



intoNotedown[k_String, v_Association]:=Block[
{box},
box = If[TrueQ[v["box"]], "Box::new(self)", "self"];
TemplateApply["
impl IntoNotedown for `type` {
    #[inline]
    fn into_node(self, span: &Span, file: &FileID) -> NotedownNode {
        `parent`::`variant`(`box`).into_node(span, file)
    }
}",
<|
"parent" -> v["parent"],
"variant" -> v["variant"],
"type"->k,
"box" -> box
|>
]
]

constructorNotedown[k_String, v_Association]:=Block[
{box, parent,selfGet,selfMut},
box = If[TrueQ[v["box"]], "Box::new(value)", "value"];
parent = v["parent"];
selfGet = If[parent=="NotedownKind","self", "self.get_"<>subtyping[parent]["constructor"]<>"()?"];
selfMut = If[parent=="NotedownKind","self", "self.mut_"<>subtyping[parent]["constructor"]<>"()?"];
TemplateApply["    /// Constructor of [`type_ref`]
    #[inline]
    pub fn `constructor`(value: `type`, span: &Span, file: &FileID) -> NotedownNode {
        `parent`::`variant`(`box`).into_node(span, file)
    }
    /// Getter of [`type_ref`]
    pub fn get_`constructor`(&self) -> Option<&`type`> {
        match `self_get` {
            `parent`::`variant`(v) => Some(v),
            _ => None,
        }
    }
    /// Mutable getter of [`type_ref`]
    pub fn mut_`constructor`(&mut self) -> Option<&mut `type`> {
        match `self_mut` {
            `parent`::`variant`(v) => Some(v),
            _ => None,
        }
    }",
    <|
"parent" -> v["parent"],
"variant" -> v["variant"],
"type"->k,
"type_ref"->"`"<>k<>"`",
"constructor"->v["constructor"],
"box" -> box,
"self_get"-> selfGet,
"self_mut"-> selfMut
|>
]
]


SetDirectory↯NotebookDirectory[];
code = TemplateApply["
use super::*;

`IntoNotedown`

impl NotedownKind {
`NotedownKind`
}
",
<|
"IntoNotedown" -> StringRiffle[KeyValueMap[intoNotedown, subtyping],"\n"],
"NotedownKind" -> StringRiffle[KeyValueMap[constructorNotedown, subtyping],"\n"]
|>
];
Export["subtyping.rs", code, "Text"]
