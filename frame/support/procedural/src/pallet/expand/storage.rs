// This file is part of Substrate.

// Copyright (C) 2020 Parity Technologies (UK) Ltd.
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
use crate::pallet::parse::storage::{Metadata, QueryKind};
use frame_support_procedural_tools::clean_type_string;

/// Generate the prefix_ident related the the storage.
/// prefix_ident is used for the prefix struct to be given to storage as first generic param.
fn prefix_ident(storage_ident: &syn::Ident) -> syn::Ident {
	syn::Ident::new(&format!("_GeneratedPrefixForStorage{}", storage_ident), storage_ident.span())
}

/// * generate StoragePrefix structs (e.g. for a storage `MyStorage` a struct with the name
///   `_GeneratedPrefixForStorage$NameOfStorage` is generated and implements StorageInstance trait.
/// * generate metadatas
/// * replace the first generic `_` by the genereted prefix structure
pub fn expand_storages(def: &mut Def) -> proc_macro2::TokenStream {
	let scrate = &def.scrate();
	let frame_system = &def.system_crate();
	let type_impl_gen = &def.type_impl_generics();
	let type_use_gen = &def.type_use_generics();
	let module_ident = &def.module.module;

	// Replace first arg `_` by the generated prefix structure.
	// Add `#[allow(type_alias_bounds)]`
	for storage_def in def.storages.iter_mut() {
		let item = &mut def.item.content.as_mut().expect("Checked by def").1[storage_def.index];

		let typ_item = if let syn::Item::Type(t) = item {
			t
		} else {
			unreachable!("Checked by def");
		};

		typ_item.attrs.push(syn::parse_quote!(#[allow(type_alias_bounds)]));

		let typ_path = if let syn::Type::Path(p) = &mut *typ_item.ty {
			p
		} else {
			unreachable!("Checked by def");
		};

		let args = if let syn::PathArguments::AngleBracketed(args) =
			&mut typ_path.path.segments[0].arguments
		{
			args
		} else {
			unreachable!("Checked by def");
		};

		let prefix_ident = prefix_ident(&storage_def.ident);
		args.args[0] = syn::parse_quote!( #prefix_ident<#type_use_gen> );
	}

	let entries = def.storages.iter()
		.map(|storage| {
			let docs = &storage.docs;

			let ident = &storage.ident;
			let gen = &def.type_use_generics();
			let full_ident = quote::quote!( #ident<#gen> );

			let metadata_trait = match &storage.metadata {
				Metadata::Value { .. } =>
					quote::quote!(#scrate::storage::types::StorageValueMetadata),
				Metadata::Map { .. } =>
					quote::quote!(#scrate::storage::types::StorageMapMetadata),
				Metadata::DoubleMap { .. } =>
					quote::quote!(#scrate::storage::types::StorageDoubleMapMetadata),
			};

			let ty = match &storage.metadata {
				Metadata::Value { value } => {
					let value = clean_type_string(&quote::quote!(#value).to_string());
					quote::quote!(
						#scrate::metadata::StorageEntryType::Plain(
							#scrate::metadata::DecodeDifferent::Encode(#value)
						)
					)
				},
				Metadata::Map { key, value } => {
					let value = clean_type_string(&quote::quote!(#value).to_string());
					let key = clean_type_string(&quote::quote!(#key).to_string());
					quote::quote!(
						#scrate::metadata::StorageEntryType::Map {
							hasher: <#full_ident as #metadata_trait>::HASHER,
							key: #scrate::metadata::DecodeDifferent::Encode(#key),
							value: #scrate::metadata::DecodeDifferent::Encode(#value),
							unused: false,
						}
					)
				},
				Metadata::DoubleMap { key1, key2, value } => {
					let value = clean_type_string(&quote::quote!(#value).to_string());
					let key1 = clean_type_string(&quote::quote!(#key1).to_string());
					let key2 = clean_type_string(&quote::quote!(#key2).to_string());
					quote::quote!(
						#scrate::metadata::StorageEntryType::DoubleMap {
							hasher: <#full_ident as #metadata_trait>::HASHER1,
							key2_hasher: <#full_ident as #metadata_trait>::HASHER2,
							key1: #scrate::metadata::DecodeDifferent::Encode(#key1),
							key2: #scrate::metadata::DecodeDifferent::Encode(#key2),
							value: #scrate::metadata::DecodeDifferent::Encode(#value),
						}
					)
				}
			};

			quote::quote_spanned!(storage.ident.span() =>
				#scrate::metadata::StorageEntryMetadata {
					name: #scrate::metadata::DecodeDifferent::Encode(
						<#full_ident as #metadata_trait>::NAME
					),
					modifier: <#full_ident as #metadata_trait>::MODIFIER,
					ty: #ty,
					default: #scrate::metadata::DecodeDifferent::Encode(
						<#full_ident as #metadata_trait>::DEFAULT
					),
					documentation: #scrate::metadata::DecodeDifferent::Encode(&[ #( #docs, )* ]),
				}
			)
		});

	let getters = def.storages.iter()
		.map(|storage| if let Some(getter) = &storage.getter {
			let docs = storage.docs.iter().map(|d| quote::quote!(#[doc = #d]));

			let ident = &storage.ident;
			let gen = &def.type_use_generics();
			let full_ident = quote::quote!( #ident<#gen> );

			match &storage.metadata {
				Metadata::Value { value } => {
					let query = match storage.query_kind.as_ref().expect("Checked by def") {
						QueryKind::OptionQuery => quote::quote!(Option<#value>),
						QueryKind::ValueQuery => quote::quote!(#value),
					};
					quote::quote_spanned!(getter.span() =>
						impl<#type_impl_gen> #module_ident<#type_use_gen> {
							#( #docs )*
							pub fn #getter() -> #query {
								<#full_ident as #scrate::storage::StorageValue<#value>>::get()
							}
						}
					)
				},
				Metadata::Map { key, value } => {
					let query = match storage.query_kind.as_ref().expect("Checked by def") {
						QueryKind::OptionQuery => quote::quote!(Option<#value>),
						QueryKind::ValueQuery => quote::quote!(#value),
					};
					quote::quote_spanned!(getter.span() =>
						impl<#type_impl_gen> #module_ident<#type_use_gen> {
							#( #docs )*
							pub fn #getter<KArg>(k: KArg) -> #query where
								KArg: #scrate::codec::EncodeLike<#key>,
							{
								<#full_ident as #scrate::storage::StorageMap<#key, #value>>::get(k)
							}
						}
					)
				},
				Metadata::DoubleMap { key1, key2, value } => {
					let query = match storage.query_kind.as_ref().expect("Checked by def") {
						QueryKind::OptionQuery => quote::quote!(Option<#value>),
						QueryKind::ValueQuery => quote::quote!(#value),
					};
					quote::quote_spanned!(getter.span() =>
						impl<#type_impl_gen> #module_ident<#type_use_gen> {
							#( #docs )*
							pub fn #getter<KArg1, KArg2>(k1: KArg1, k2: KArg2) -> #query where
								KArg1: #scrate::codec::EncodeLike<#key1>,
								KArg2: #scrate::codec::EncodeLike<#key2>,
							{
								<
									#full_ident
									as #scrate::storage::StorageDoubleMap<#key1, #key2, #value>
								>::get(k1, k2)
							}
						}
					)
				},
			}
		} else {
			Default::default()
		});

	let prefix_structs = def.storages.iter().map(|storage_def| {
		let prefix_struct_ident = prefix_ident(&storage_def.ident);
		let prefix_struct_vis = &storage_def.vis;
		let prefix_struct_const = storage_def.ident.to_string();

		quote::quote_spanned!(storage_def.ident.span() =>
			#prefix_struct_vis struct #prefix_struct_ident<#type_use_gen>(
				core::marker::PhantomData<(#type_use_gen,)>
			);
			impl<#type_impl_gen> #scrate::traits::StorageInstance
			for #prefix_struct_ident<#type_use_gen>
			{
				type PalletInfo = <T as #frame_system::Trait>::PalletInfo;
				type Pallet = Module<#type_use_gen>;
				const STORAGE_PREFIX: &'static str = #prefix_struct_const;
			}
		)
	});

	quote::quote!(
		impl<#type_impl_gen> #module_ident<#type_use_gen> {
			#[doc(hidden)]
			pub fn storage_metadata() -> #scrate::metadata::StorageMetadata {
				#scrate::metadata::StorageMetadata {
					prefix: #scrate::metadata::DecodeDifferent::Encode(
						<
							<T as #frame_system::Trait>::PalletInfo as #scrate::traits::PalletInfo
						>::name::<#module_ident<#type_use_gen>>()
							.expect("Every active pallet has a name in the runtime; qed")
					),
					entries: #scrate::metadata::DecodeDifferent::Encode(
						&[ #( #entries, )* ]
					),
				}
			}
		}

		#( #getters )*
		#( #prefix_structs )*
	)
}