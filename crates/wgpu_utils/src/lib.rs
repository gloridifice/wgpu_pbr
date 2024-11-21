// extern crate proc_macro;
use proc_macro::TokenStream;
use quote::quote;
use syn::{parse::Parse, parse_macro_input};

// ``` rust
// bind_group_layout_entries! {
//     FRAGMENT => TEXTURE_2D
// }
// ```
// #[proc_macro]
// pub fn bind_group_layout_entries(input: TokenStream) -> TokenStream {
//     let input = parse_macro_input!(input);
//     quote! {

//     }.into()
// }
