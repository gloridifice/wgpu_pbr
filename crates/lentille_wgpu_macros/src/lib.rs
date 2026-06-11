use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::{format_ident, quote};
use syn::{
    Ident, LitInt, Token, Type, bracketed,
    parse::{Parse, ParseStream},
    parse_macro_input,
    punctuated::Punctuated,
};

enum Stage {
    Frag,
    Vert,
    All,
}

impl Stage {
    fn tokens(&self) -> TokenStream2 {
        match self {
            Stage::Frag => quote!(wgpu::ShaderStages::FRAGMENT),
            Stage::Vert => quote!(wgpu::ShaderStages::VERTEX),
            Stage::All => quote!(wgpu::ShaderStages::all()),
        }
    }
}

struct Entry {
    index: LitInt,
    stage: Stage,
    field: Ident,
    ty: Type,
}

impl Parse for Entry {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let index: LitInt = input.parse()?;
        input.parse::<Token![:]>()?;

        let stage_ident: Ident = input.parse()?;
        let stage = match stage_ident.to_string().as_str() {
            "frag" => Stage::Frag,
            "vert" => Stage::Vert,
            "all" => Stage::All,
            _ => {
                return Err(syn::Error::new(
                    stage_ident.span(),
                    "expected `frag`, `vert` or `all`",
                ));
            }
        };

        input.parse::<Token![=>]>()?;
        let field: Ident = input.parse()?;
        input.parse::<Token![:]>()?;
        let ty: Type = input.parse()?;

        Ok(Self {
            index,
            stage,
            field,
            ty,
        })
    }
}

struct BindingDefine {
    name: Ident,
    entries: Vec<Entry>,
}

impl Parse for BindingDefine {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let content;
        bracketed!(content in input);
        let name: Ident = content.parse()?;

        let entries: Punctuated<Entry, Token![,]> = Punctuated::parse_terminated(input)?;

        Ok(Self {
            name,
            entries: entries.into_iter().collect(),
        })
    }
}

/// Generate a typed bind group layout + builder from a compact description.
///
/// ```ignore
/// binding_define! {
///     [Xxx]
///     0: frag => aaa: TypedTextureView<Dim2D, SampleFloatFilterable>,
///     1: frag => bbb: TypedTextureView<Dim2D, SampleFloatFilterable>,
/// }
/// ```
///
/// Each field type must implement
/// [`lentille_wgpu_utils::typed_binding_resource::TypedBinding`], which yields
/// both the `wgpu::BindingType` (type-level, for the layout) and the
/// `wgpu::BindingResource` (value-level, for the bind group).
#[proc_macro]
pub fn binding_define(input: TokenStream) -> TokenStream {
    let BindingDefine { name, entries } = parse_macro_input!(input as BindingDefine);

    let builder = format_ident!("{}BindGroupBuilder", name);
    let layout = format_ident!("{}BindGroupLayout", name);
    let bgl_label = format!("{name} BGL");
    let bg_label = format!("{name} BG");

    let fields = entries.iter().map(|e| {
        let field = &e.field;
        let ty = &e.ty;
        quote! { pub #field: &'a #ty, }
    });

    let layout_entries = entries.iter().map(|e| {
        let index = &e.index;
        let visibility = e.stage.tokens();
        let ty = &e.ty;
        quote! {
            wgpu::BindGroupLayoutEntry {
                binding: #index,
                visibility: #visibility,
                ty: <#ty as lentille_wgpu_utils::typed_binding_resource::TypedBinding>::binding_layout_type(),
                count: None,
            }
        }
    });

    let bg_entries = entries.iter().map(|e| {
        let index = &e.index;
        let field = &e.field;
        quote! {
            wgpu::BindGroupEntry {
                binding: #index,
                resource: lentille_wgpu_utils::typed_binding_resource::TypedBinding::as_binding_resource(self.#field),
            }
        }
    });

    let expanded = quote! {
        pub struct #builder<'a> {
            #(#fields)*
        }

        pub struct #layout(pub wgpu::BindGroupLayout);

        impl #layout {
            pub fn new(device: &wgpu::Device) -> Self {
                let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: Some(#bgl_label),
                    entries: &[
                        #(#layout_entries),*
                    ],
                });
                Self(layout)
            }
        }

        impl ::core::ops::Deref for #layout {
            type Target = wgpu::BindGroupLayout;

            fn deref(&self) -> &Self::Target {
                &self.0
            }
        }

        impl<'a> #builder<'a> {
            pub fn build(self, device: &wgpu::Device, layout: &#layout) -> wgpu::BindGroup {
                device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some(#bg_label),
                    layout: &layout.0,
                    entries: &[
                        #(#bg_entries),*
                    ],
                })
            }
        }
    };

    expanded.into()
}
