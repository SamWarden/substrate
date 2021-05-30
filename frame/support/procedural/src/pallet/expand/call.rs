// This file is part of Substrate.

// Copyright (C) 2020-2021 Parity Technologies (UK) Ltd.
// SPDX-License-Identifier: Apache-2.0

// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// 	http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use crate::pallet::Def;
use frame_support_procedural_tools::clean_type_string;
use std::cell::RefCell;
use syn::spanned::Spanned;

struct Counter(u64);

impl Counter {
	fn inc(&mut self) -> u64 {
		let ret = self.0;
		self.0 += 1;
		ret
	}
}

thread_local!{
	/// Counter to generate a relatively unique identifier for __is_call_part_defined macro.
	/// This is necessary because declarative macros gets hoisted to the crate root, which may
	/// clash with other pallets' __is_call_part_defined macro.
	static COUNTER: RefCell<Counter> = RefCell::new(Counter(0));
}

/// * Generate enum call and implement various trait on it.
/// * Implement Callable and call_function on `Pallet`
pub fn expand_call(def: &mut Def) -> proc_macro2::TokenStream {
	let (span, where_clause, methods, docs) = match def.call.as_ref() {
		Some(call) => {
			let span = call.attr_span;
			let where_clause = call.where_clause.clone();
			let methods = call.methods.clone();
			let docs = call.docs.clone();

			(span, where_clause, methods, docs)
		}
		None => (def.item.span(), None, Vec::new(), Vec::new()),
	};
	let frame_support = &def.frame_support;
	let frame_system = &def.frame_system;
	let type_impl_gen = &def.type_impl_generics(span);
	let type_decl_bounded_gen = &def.type_decl_bounded_generics(span);
	let type_use_gen = &def.type_use_generics(span);
	let call_ident = syn::Ident::new("Call", span);
	let pallet_ident = &def.pallet_struct.pallet;

	let fn_name = methods.iter().map(|method| &method.name).collect::<Vec<_>>();

	let fn_weight = methods.iter().map(|method| &method.weight);

	let fn_doc = methods.iter().map(|method| &method.docs).collect::<Vec<_>>();

	let args_name = methods.iter()
		.map(|method| method.args.iter().map(|(_, name, _)| name.clone()).collect::<Vec<_>>())
		.collect::<Vec<_>>();

	let args_type = methods.iter()
		.map(|method| method.args.iter().map(|(_, _, type_)| type_.clone()).collect::<Vec<_>>())
		.collect::<Vec<_>>();

	let args_compact_attr = methods.iter().map(|method| {
		method.args.iter()
			.map(|(is_compact, _, type_)| {
				if *is_compact {
					quote::quote_spanned!(type_.span() => #[codec(compact)] )
				} else {
					quote::quote!()
				}
			})
			.collect::<Vec<_>>()
	});

	let args_metadata_type = methods.iter().map(|method| {
		method.args.iter()
			.map(|(is_compact, _, type_)| {
				let final_type = if *is_compact {
					quote::quote_spanned!(type_.span() => Compact<#type_>)
				} else {
					quote::quote!(#type_)
				};
				clean_type_string(&final_type.to_string())
			})
			.collect::<Vec<_>>()
	});

	let default_docs = [syn::parse_quote!(
		r"Contains one variant per dispatchable that can be called by an extrinsic."
	)];
	let docs = if docs.is_empty() {
		&default_docs[..]
	} else {
		&docs[..]
	};

	let maybe_compile_error = if def.call.is_none() {
		quote::quote!{
			compile_error!("Pallet does not have #[pallet::call] defined. Did you forget to include it?");
		}
	} else {
		proc_macro2::TokenStream::new()
	};

	let count = COUNTER.with(|counter| counter.borrow_mut().inc());
	let macro_ident = syn::Ident::new(&format!("__is_call_part_defined_{}", count), span);

	quote::quote_spanned!(span =>
		#[macro_export]
		macro_rules! #macro_ident {
			() => {
				#maybe_compile_error
			};
		}

		pub use #macro_ident as __is_call_part_defined;

		#( #[doc = #docs] )*
		#[derive(
			#frame_support::RuntimeDebugNoBound,
			#frame_support::CloneNoBound,
			#frame_support::EqNoBound,
			#frame_support::PartialEqNoBound,
			#frame_support::codec::Encode,
			#frame_support::codec::Decode,
		)]
		#[codec(encode_bound())]
		#[codec(decode_bound())]
		#[allow(non_camel_case_types)]
		pub enum #call_ident<#type_decl_bounded_gen> #where_clause {
			#[doc(hidden)]
			#[codec(skip)]
			__Ignore(
				#frame_support::sp_std::marker::PhantomData<(#type_use_gen,)>,
				#frame_support::Never,
			),
			#( #( #[doc = #fn_doc] )* #fn_name( #( #args_compact_attr #args_type ),* ), )*
		}

		impl<#type_impl_gen> #frame_support::dispatch::GetDispatchInfo
			for #call_ident<#type_use_gen>
			#where_clause
		{
			fn get_dispatch_info(&self) -> #frame_support::dispatch::DispatchInfo {
				match *self {
					#(
						Self::#fn_name ( #( ref #args_name, )* ) => {
							let __pallet_base_weight = #fn_weight;

							let __pallet_weight = <
								dyn #frame_support::dispatch::WeighData<( #( & #args_type, )* )>
							>::weigh_data(&__pallet_base_weight, ( #( #args_name, )* ));

							let __pallet_class = <
								dyn #frame_support::dispatch::ClassifyDispatch<
									( #( & #args_type, )* )
								>
							>::classify_dispatch(&__pallet_base_weight, ( #( #args_name, )* ));

							let __pallet_pays_fee = <
								dyn #frame_support::dispatch::PaysFee<( #( & #args_type, )* )>
							>::pays_fee(&__pallet_base_weight, ( #( #args_name, )* ));

							#frame_support::dispatch::DispatchInfo {
								weight: __pallet_weight,
								class: __pallet_class,
								pays_fee: __pallet_pays_fee,
							}
						},
					)*
					Self::__Ignore(_, _) => unreachable!("__Ignore cannot be used"),
				}
			}
		}

		impl<#type_impl_gen> #frame_support::dispatch::GetCallName for #call_ident<#type_use_gen>
			#where_clause
		{
			fn get_call_name(&self) -> &'static str {
				match *self {
					#( Self::#fn_name(..) => stringify!(#fn_name), )*
					Self::__Ignore(_, _) => unreachable!("__PhantomItem cannot be used."),
				}
			}

			fn get_call_names() -> &'static [&'static str] {
				&[ #( stringify!(#fn_name), )* ]
			}
		}

		impl<#type_impl_gen> #frame_support::traits::UnfilteredDispatchable
			for #call_ident<#type_use_gen>
			#where_clause
		{
			type Origin = #frame_system::pallet_prelude::OriginFor<T>;
			fn dispatch_bypass_filter(
				self,
				origin: Self::Origin
			) -> #frame_support::dispatch::DispatchResultWithPostInfo {
				match self {
					#(
						Self::#fn_name( #( #args_name, )* ) => {
							#frame_support::sp_tracing::enter_span!(
								#frame_support::sp_tracing::trace_span!(stringify!(#fn_name))
							);
							<#pallet_ident<#type_use_gen>>::#fn_name(origin, #( #args_name, )* )
								.map(Into::into).map_err(Into::into)
						},
					)*
					Self::__Ignore(_, _) => {
						let _ = origin; // Use origin for empty Call enum
						unreachable!("__PhantomItem cannot be used.");
					},
				}
			}
		}

		impl<#type_impl_gen> #frame_support::dispatch::Callable<T> for #pallet_ident<#type_use_gen>
			#where_clause
		{
			type Call = #call_ident<#type_use_gen>;
		}

		impl<#type_impl_gen> #pallet_ident<#type_use_gen> #where_clause {
			#[doc(hidden)]
			#[allow(dead_code)]
			pub fn call_functions() -> &'static [#frame_support::dispatch::FunctionMetadata] {
				&[ #(
					#frame_support::dispatch::FunctionMetadata {
						name: #frame_support::dispatch::DecodeDifferent::Encode(
							stringify!(#fn_name)
						),
						arguments: #frame_support::dispatch::DecodeDifferent::Encode(
							&[ #(
								#frame_support::dispatch::FunctionArgumentMetadata {
									name: #frame_support::dispatch::DecodeDifferent::Encode(
										stringify!(#args_name)
									),
									ty: #frame_support::dispatch::DecodeDifferent::Encode(
										#args_metadata_type
									),
								},
							)* ]
						),
						documentation: #frame_support::dispatch::DecodeDifferent::Encode(
							&[ #( #fn_doc ),* ]
						),
					},
				)* ]
			}
		}
	)
}
