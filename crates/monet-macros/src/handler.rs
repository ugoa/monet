use proc_macro2::{Span, TokenStream};
use quote::{ToTokens, quote};
use syn::{Ident, ImplItem, Item, Pat, ReturnType, Signature, Type};
