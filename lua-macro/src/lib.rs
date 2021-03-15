#![feature(asm)]
#![feature(const_loop)]
#![feature(const_if_match)]
#![allow(unused_mut)]
#![allow(dead_code)]
use syn::{self, punctuated};
use syn::parse::{Parse, ParseStream};
use syn::{parenthesized, bracketed, token, Token};
use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::{quote, ToTokens};
// use smash::hash40;
//use std::io::Write;

// borrowed from skyline-smash
macro_rules! reflect {
    ($bits:expr, $value:expr) => {{
        let mut reflection = 0;
        let mut value = $value;
        let mut i = 0;

        while i < $bits {
            if (value & 0x01) == 1 {
                reflection |= 1 << (($bits - 1) - i)
            }

            value >>= 1;
            i += 1;
        }

        reflection
    }};
}

const fn make_table(poly: u32) -> [u32; 256] {
    let mut table = [0; 256];
    let top_bit = 1 << 31;
    let mut byte;

    let mut i = 0;
    while i <= 255 {
        byte = reflect!(8, i);

        let mut value = byte << 24;

        let mut j = 0;
        while j < 8 {
            if (value & top_bit) != 0 {
                value = (value << 1) ^ poly
            } else {
                value <<= 1
            }

            j += 1;
        }

        value = reflect!(32, value);

        table[i as usize] = value;

        i += 1;
    }

    table
}

const IEEE_TABLE: [u32; 256] = make_table(0x04C11DB7);

const fn crc32(bytes: &[u8]) -> u32 {
    let mut value = !0u32;
    let mut i = 0;
    while i < bytes.len() {
        value = (value >> 8) ^ (IEEE_TABLE[((value ^ (bytes[i] as u32)) & 0xFF) as usize]);
        i += 1;
    }

    !value
}

const fn hash40(string: &str) -> u64 {
    let bytes = string.as_bytes();

    ((bytes.len() as u64) << 32) + crc32(bytes) as u64
}

struct ScriptAttrs {
    pub agent: syn::LitStr,
    pub scripts: Vec<syn::LitStr>,
    pub category: syn::Path
}

struct StatusAttrs {
    pub agent: syn::LitStr,
    pub status: syn::Path,
    pub condition: syn::Path
}

#[derive(Debug, Clone)]
struct MetaItem<Keyword: Parse, Item: Parse> {
    pub ident: Keyword,
    pub item: Item,
}

impl<Keyword: Parse, Item: Parse> Parse for MetaItem<Keyword, Item> {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let ident = input.parse()?;
        let item = if input.peek(token::Paren) {
            let content;
            parenthesized!(content in input);
            content.parse()?
        } else if input.peek(token::Bracket)  {
            let content;
            bracketed!(content in input);
            content.parse()?
        } else {
            input.parse::<Token![=]>()?;
            input.parse()?
        };

        Ok(Self {
            ident,
            item
        })
    }
}

#[derive(Debug, Clone)]
struct BracketedList<Keyword: Parse, Item: Parse, Punctuation: Parse> {
    pub ident: Keyword,
    pub list: punctuated::Punctuated<Item, Punctuation>
}

impl<Keyword: Parse, Item: Parse, Punctuation: Parse> Parse for BracketedList<Keyword, Item, Punctuation> {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let ident = input.parse()?;
        let _: Token![=] = input.parse()?;
        let list = if input.peek(token::Bracket) {
            let content;
            bracketed!(content in input);
            content.parse_terminated(Item::parse)?
        } else {
            return Err(input.error("could not find bracketed list"));
        };
        Ok(Self {
            ident: ident,
            list: list
        })
    }
}

mod kw {
    syn::custom_keyword!(agent);
    syn::custom_keyword!(script);
    syn::custom_keyword!(scripts);
    syn::custom_keyword!(category);
    syn::custom_keyword!(status);
    syn::custom_keyword!(condition);
}

impl syn::parse::Parse for ScriptAttrs {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let look = input.lookahead1();
        let agent: syn::LitStr = if look.peek(kw::agent) {
            let MetaItem::<kw::agent, syn::LitStr> { item: string, .. } = input.parse()?;
            
            string
        } else {
            return Err(look.error());
        };

        let _: syn::Token![,] = input.parse()?;
        let look = input.lookahead1();

        
        let scripts: Vec<syn::LitStr> = if look.peek(kw::script) {
            let MetaItem::<kw::script, syn::LitStr> { item: string, .. } = input.parse()?;
            
            vec![string]
        } else if look.peek(kw::scripts) {
            let BracketedList::<kw::scripts, syn::LitStr, syn::Token![,]> { list: script_names, .. } = input.parse()?;

            let mut s = Vec::new();
            for script in script_names.iter() {
                s.push(script.clone());
            }
            s
        } else {
            return Err(look.error());
        };

        let _: syn::Token![,] = input.parse()?;
        let look = input.lookahead1();

        let category: syn::Path = if look.peek(kw::category) {
            let MetaItem::<kw::category, syn::Path> { item: cat, .. } = input.parse()?;

            cat
        }
        else {
            return Err(look.error());
        };
        Ok(Self {
            agent: agent,
            scripts: scripts,
            category: category
        })
    }
}

impl syn::parse::Parse for StatusAttrs {
    fn parse(input : syn::parse::ParseStream) -> syn::Result<Self> {
        let look = input.lookahead1();
        let agent: syn::LitStr = if look.peek(kw::agent) {
            let MetaItem::<kw::agent, syn::LitStr> { item: string, .. } = input.parse()?;

            string
        } else {
            return Err(look.error());
        };

        let _: syn::Token![,] = input.parse()?;
        let look = input.lookahead1();

        let status: syn::Path = if look.peek(kw::status) {
            let MetaItem::<kw::status, syn::Path> { item: path, .. } = input.parse()?;

            path
        } else {
            return Err(look.error());
        };

        let _: syn::Token![,] = input.parse()?;
        let look = input.lookahead1();

        let condition: syn::Path = if look.peek(kw::condition) {
            let MetaItem::<kw::condition, syn::Path> { item: cond, .. } = input.parse()?;

            cond
        } else {
            return Err(look.error());
        };
        Ok(Self {
            agent: agent,
            status: status,
            condition: condition
        })
    }
}

#[proc_macro]
pub fn replace_script(input: TokenStream) -> TokenStream {
    let ident = syn::parse_macro_input!(input as syn::Ident);
    let installer_name = quote::format_ident!("_lua_replace_script_install_{}", ident);
    quote!(
        unsafe { #installer_name(); }
    ).into()
}

#[proc_macro]
pub fn replace_status_script(input: TokenStream) -> TokenStream {
    let ident = syn::parse_macro_input!(input as syn::Ident);
    let installer_name = quote::format_ident!("_lua_replace_status_install_{}", ident);
    quote!(
        unsafe { #installer_name(); }
    ).into()
}

#[proc_macro]
pub fn replace_fighter_frame(input: TokenStream) -> TokenStream {
    let ident = syn::parse_macro_input!(input as syn::Ident);
    let installer_name = quote::format_ident!("_lua_replace_sys_line_fighter_install_{}", ident);
    quote!(
        unsafe { #installer_name(); }
    ).into()
}

#[proc_macro]
pub fn replace_weapon_frame(input: TokenStream) -> TokenStream {
    let ident = syn::parse_macro_input!(input as syn::Ident);
    let installer_name = quote::format_ident!("_lua_replace_sys_line_weapon_install_{}", ident);
    quote!(
        unsafe { #installer_name(); }
    ).into()
}

struct SysLineAttrs {
    pub agent: syn::Path
}

impl syn::parse::Parse for SysLineAttrs {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        if input.peek(kw::agent) {
            let MetaItem::<kw::agent, syn::Path> { item: kind, .. } = input.parse()?;

            Ok(Self { agent: kind })
        } else {
            Err(input.error("no agent token found"))
        }
    }
}

#[proc_macro_attribute]
pub fn fighter_frame(attr: TokenStream, item: TokenStream) -> TokenStream {
    let attrs = syn::parse_macro_input!(attr as SysLineAttrs);
    let item_clone = item.clone();
    let usr_fn = syn::parse_macro_input!(item as syn::ItemFn);

    let _agent = attrs.agent;
    let usr_fn_name = &usr_fn.sig.ident;

    let _orig_fn = usr_fn.block.to_token_stream();
    let replace_name = quote::format_ident!("_lua_replace_sys_line_fighter_replace_{}", usr_fn_name);
    let install_name = quote::format_ident!("_lua_replace_sys_line_fighter_install_{}", usr_fn_name);

    let mut output = TokenStream2::from(item_clone);

    quote!(
        #[allow(unused_unsafe)]
        pub unsafe extern "C" fn #replace_name(fighter: &mut smash::lua2cpp::L2CFighterCommon) -> smash::lib::L2CValue {
            #usr_fn_name(fighter);

            smash::lua2cpp::L2CFighterCommon_sys_line_system_control_fighter(fighter)
        }

        pub unsafe fn #install_name() {
            smash_script::replace_sys_line_fighter_script(#_agent, #replace_name);
        }
    ).to_tokens(&mut output);

    output.into()
}

#[proc_macro_attribute]
pub fn weapon_frame(attr: TokenStream, item: TokenStream) -> TokenStream {
    let attrs = syn::parse_macro_input!(attr as SysLineAttrs);
    let item_clone = item.clone();
    let usr_fn = syn::parse_macro_input!(item as syn::ItemFn);

    let _agent = attrs.agent;
    let usr_fn_name = &usr_fn.sig.ident;

    let _orig_fn = usr_fn.block.to_token_stream();
    let replace_name = quote::format_ident!("_lua_replace_sys_line_weapon_replace_{}", usr_fn_name);
    let install_name = quote::format_ident!("_lua_replace_sys_line_weapon_install_{}", usr_fn_name);

    let mut output = TokenStream2::from(item_clone);

    quote!(
        #[allow(unused_unsafe)]
        pub unsafe extern "C" fn #replace_name(fighter: &mut smash::lua2cpp::L2CFighterBase) -> smash::lib::L2CValue {
            #usr_fn_name(fighter);

            smash::lua2cpp::L2CFighterBase_sys_line_system_control(fighter) // call original C function just because /shrug
        }

        unsafe fn #install_name() {
            smash_script::replace_sys_line_weapon_script(#_agent, #replace_name);
        }
    ).to_tokens(&mut output);

    output.into()
}

#[proc_macro_attribute]
pub fn script(attr: TokenStream, item: TokenStream) -> TokenStream {
    let attr = syn::parse_macro_input!(attr as ScriptAttrs);
    let item_clone = item.clone();
    let usr_fn = syn::parse_macro_input!(item as syn::ItemFn);

    let usr_fn_name = &usr_fn.sig.ident;
    let _agent = attr.agent;
    let _scripts = attr.scripts;

    let bootstrapper_name = quote::format_ident!("_lua_replace_script_bootstrapper_{}", usr_fn_name);
    let internal_name = quote::format_ident!("_lua_replace_script_internal_{}", usr_fn_name);
    let usr_new_name = quote::format_ident!("_lua_replace_script_usr_{}", usr_fn_name);
    let installer_name = quote::format_ident!("_lua_replace_script_install_{}", usr_fn_name);

    // simpler to do it this way imo
    let func_string = item_clone.to_string();
    let func_string = func_string.replace(&format!("fn {}", usr_fn_name.to_string()), &format!("fn {}", usr_new_name.to_string()));
    let func_string = "#[inline(always)]\n".to_owned() + &func_string;
    let mut output: TokenStream2 = func_string.parse().unwrap();

    let mut replace_strings: Vec<String> = Vec::new();
    replace_strings.resize(_scripts.len(), "".to_string());

    let agent_name = _agent.value();
    let agent_hash;
    if agent_name.starts_with("0x") {
        agent_hash = format!("smash::phx::Hash40::new_raw({})", agent_name);
    }
    else {
        agent_hash = format!("smash::phx::Hash40::new(\"{}\")", agent_name);
    }
    for x in 0..replace_strings.len() {
        let current_string = replace_strings.get_mut(x).unwrap();
        let current_script = _scripts.get(x).unwrap().value();
        let current_script_hash;
        if current_script.starts_with("0x") {
            current_script_hash = format!("smash::phx::Hash40::new_raw({})", current_script);
        }
        else {
            current_script_hash = format!("smash::phx::Hash40::new(\"{}\")", current_script);
        }
        *current_string = format!("smash_script::replace_lua_script({}, {}, {}, {});", agent_hash, current_script_hash, bootstrapper_name.to_string(), attr.category.get_ident().unwrap().to_string());
    }

    let mut installer_string = format!(r#"
    #[allow(non_uppercase_globals)]
    unsafe fn {}() {{"#, installer_name.to_string());
    for x in replace_strings.iter() {
        installer_string += x;
    }
    installer_string += " }";
    quote!(
        #[inline(never)]
        #[allow(unused_unsafe)]
        unsafe fn #internal_name(l2c_ret: &mut smash::lib::L2CValue, fighter: &mut smash::lua2cpp::L2CAgentBase) {
            fighter.clear_lua_stack();
            #usr_new_name(fighter);
            *l2c_ret = smash::lib::L2CValue::new_int(0);
            asm!(r#"
            b #0x8
            .byte 0xE5, 0xB1, 0x00, 0xB0
            "#);
        }

        #[inline(never)]
        #[allow(unused_unsafe)]
        unsafe extern "C" fn #bootstrapper_name(fighter: &mut smash::lua2cpp::L2CAgentBase, variadic: &mut smash::lib::utility::Variadic) {
            let format = variadic.get_format();
            let mut value = smash::lib::L2CValue::new();
            if format == 0 as *const skyline::libc::c_char {
                #internal_name(&mut value, fighter);
            }
            else {
                #internal_name(&mut value, fighter);
                value.push_variadic(0, format, variadic);
                println!("variadic one");
            }
            asm!(r#"
            b #0x8
            .byte 0xE5, 0xB1, 0x00, 0xB0
            "#);
        }
    ).to_tokens(&mut output);
    installer_string.parse::<TokenStream2>().unwrap().to_tokens(&mut output);
    output.into()
}

#[proc_macro_attribute]
pub fn status(attr: TokenStream, item: TokenStream) -> TokenStream {
    let attr = syn::parse_macro_input!(attr as StatusAttrs);
    let item_clone = item.clone();
    let usr_fn = syn::parse_macro_input!(item as syn::ItemFn);

    let usr_fn_name = &usr_fn.sig.ident;
    let _agent = attr.agent;
    let _status = attr.status;
    let _condition = attr.condition;

    let bootstrapper_name = quote::format_ident!("_lua_replace_status_bootstrapper_{}", usr_fn_name); // still required because of exceptions
    let usr_new_name = quote::format_ident!("_lua_replace_status_usr_{}", usr_fn_name);
    let installer_name = quote::format_ident!("_lua_replace_status_install_{}", usr_fn_name);

    let func_string = item_clone.to_string();
    let func_string = func_string.replace(&format!("fn {}", usr_fn_name.to_string()), &format!("fn {}", usr_new_name.to_string()));
    let func_string = "#[inline(always)]\n".to_owned() + &func_string;
    let mut output: TokenStream2 = func_string.parse().unwrap();

    let agent_name = _agent.value();
    let agent_hash;
    if agent_name.starts_with("0x") {
        agent_hash = format!("smash::phx::Hash40::new_raw({})", agent_name);
    }
    else {
        agent_hash = format!("smash::phx::Hash40::new(\"{}\")", agent_name);
    }

    let replace_string = format!("smash_script::replace_status_script({}, {}, {}, {});", agent_hash, _status.get_ident().unwrap().to_string(), _condition.get_ident().unwrap().to_string(), bootstrapper_name.to_string());
    let mut installer_string = format!(r#"
    #[allow(non_uppercase_globals)]
    unsafe fn {}() {{
        {}
    }}
    "#, installer_name.to_string(), replace_string);

    quote!(
        #[inline(never)]
        #[allow(unused_unsafe)]
        unsafe extern "C" fn #bootstrapper_name(fighter: &mut smash::lua2cpp::L2CFighterBase) -> smash::lib::L2CValue {
            #usr_new_name(std::mem::transmute(fighter))
        }
    ).to_tokens(&mut output);
    installer_string.parse::<TokenStream2>().unwrap().to_tokens(&mut output);
    output.into()
}