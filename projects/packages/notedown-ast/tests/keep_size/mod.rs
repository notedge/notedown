// use notedown_ast::{
//     nodes::{ListView, MathNode, SmartLink, TableView, TextSpan},
//     value::NotedownType,
//     NotedownKind, NotedownNode, NotedownValue,
// };
// use std::mem::size_of;
//
// macro_rules! show_size {
//     () => {
//         println!("{:<22} {:>4}    {}", "Type", "T", "Option<T>");
//     };
//     ($t:ty) => {
//         println!("{:<22} {:4} {:>12}", stringify!($t), size_of::<$t>(), size_of::<Option<$t>>())
//     };
// }
//
// #[test]
// fn keep_size() {
//     show_size!();
//     show_size!(NotedownNode);
//     show_size!(NotedownKind);
//     show_size!(NotedownValue);
//     show_size!(NotedownType);
//
//     show_size!(ListView);
//     show_size!(TableView);
//     show_size!(SmartLink);
//     show_size!(MathNode);
//     show_size!(TextSpan);
//
//     assert_eq!(size_of::<NotedownNode>(), 56);
//     assert_eq!(size_of::<NotedownKind>(), 32);
//     assert_eq!(size_of::<NotedownValue>(), 80);
// }
