# Dependencias de Rust que se distribuyen con Pliego

Instantánea generada de `Cargo.lock` para `x86_64-pc-windows-msvc` (solo dependencias normales, sin las de desarrollo ni
las de compilación) de los tres ejecutables: `app` (`pliego-app`), `cli` (`pliego`) y `worker` (`pliego-worker`).
Son 514 crates de terceros, todos con licencia permisiva o con copyleft solo por archivo (MPL-2.0). `cargo deny check licenses`
lo comprueba en cada PR. Cuando un crate ofrece varias licencias (`A OR B`), Pliego lo usa bajo la más permisiva de las
dos; la columna «Licencia» es la declarada por el crate.

Para regenerarla:

```
cargo tree --locked -p pliego-app -e normal --target x86_64-pc-windows-msvc --prefix none --format "{p}|{l}"
```

| Crate | Versión | Licencia | Ejecutables |
| --- | --- | --- | --- |
| adler2 | 2.0.1 | 0BSD OR MIT OR Apache-2.0 | app, cli, worker |
| aead | 0.6.1 | MIT OR Apache-2.0 | app, cli |
| aes | 0.9.4 | MIT OR Apache-2.0 | app, cli, worker |
| aes-gcm | 0.11.1 | Apache-2.0 OR MIT | app, cli |
| aho-corasick | 1.1.5 | Unlicense OR MIT | app, cli |
| aligned-vec | 0.6.4 | MIT | app, cli, worker |
| alloc-no-stdlib | 2.0.4 | BSD-3-Clause | app, cli, worker |
| alloc-no-stdlib | 3.0.0 | BSD-3-Clause | app |
| alloc-stdlib | 0.2.4 | BSD-3-Clause | app, cli, worker |
| alloc-stdlib | 0.3.0 | BSD-3-Clause | app |
| allocator-api2 | 0.2.21 | MIT OR Apache-2.0 | app, cli, worker |
| anstream | 1.0.0 | MIT OR Apache-2.0 | cli |
| anstyle | 1.0.14 | MIT OR Apache-2.0 | cli |
| anstyle-parse | 1.0.0 | MIT OR Apache-2.0 | cli |
| anstyle-query | 1.1.5 | MIT OR Apache-2.0 | cli |
| anstyle-wincon | 3.0.11 | MIT OR Apache-2.0 | cli |
| anyhow | 1.0.104 | MIT OR Apache-2.0 | app, cli |
| arc-swap | 1.9.3 | MIT OR Apache-2.0 | app, cli |
| arrayvec | 0.7.8 | MIT OR Apache-2.0 | app, cli, worker |
| asn1-rs | 0.7.2 | MIT OR Apache-2.0 | app, cli |
| asn1-rs-derive | 0.6.0 | MIT OR Apache-2.0 | app, cli |
| asn1-rs-impl | 0.2.0 | MIT/Apache-2.0 | app, cli |
| async-channel | 2.5.0 | Apache-2.0 OR MIT | app, cli, worker |
| async-trait | 0.1.92 | MIT OR Apache-2.0 | app, cli |
| base16ct | 0.2.0 | Apache-2.0 OR MIT | app, cli |
| base64 | 0.22.1 | MIT OR Apache-2.0 | app |
| base64 | 0.23.1 | MIT OR Apache-2.0 | app, cli |
| base64ct | 1.8.3 | Apache-2.0 OR MIT | app, cli |
| bit-set | 0.8.0 | Apache-2.0 OR MIT | app |
| bit-vec | 0.8.0 | Apache-2.0 OR MIT | app |
| bitflags | 1.3.2 | MIT/Apache-2.0 | app |
| bitflags | 2.13.2 | MIT OR Apache-2.0 | app, cli, worker |
| bitpacking | 0.9.3 | MIT | app, cli |
| block-buffer | 0.10.4 | MIT OR Apache-2.0 | app, cli |
| block-buffer | 0.12.1 | MIT OR Apache-2.0 | app, cli, worker |
| block-padding | 0.4.2 | MIT OR Apache-2.0 | app, cli, worker |
| boa_ast | 0.22.0 | Unlicense OR MIT | app, cli, worker |
| boa_engine | 0.22.0 | Unlicense OR MIT | app, cli, worker |
| boa_gc | 0.22.0 | Unlicense OR MIT | app, cli, worker |
| boa_interner | 0.22.0 | Unlicense OR MIT | app, cli, worker |
| boa_macros | 0.22.0 | Unlicense OR MIT | app, cli, worker |
| boa_parser | 0.22.0 | Unlicense OR MIT | app, cli, worker |
| boa_string | 0.22.0 | Unlicense OR MIT | app, cli, worker |
| bon | 3.10.3 | MIT OR Apache-2.0 | app, cli |
| bon-macros | 3.10.3 | MIT OR Apache-2.0 | app, cli |
| brotli | 9.1.1 | BSD-3-Clause AND MIT | app |
| brotli-decompressor | 5.0.3 | BSD-3-Clause/MIT | app, cli, worker |
| brotli-decompressor | 6.2.0 | BSD-3-Clause/MIT | app |
| bumpalo | 3.20.3 | MIT OR Apache-2.0 | app, cli |
| bytemuck | 1.25.2 | Zlib OR Apache-2.0 OR MIT | app, cli, worker |
| bytemuck_derive | 1.12.1 | Zlib OR Apache-2.0 OR MIT | app, cli, worker |
| byteorder | 1.5.0 | Unlicense OR MIT | app, cli, worker |
| byteorder-lite | 0.1.0 | Unlicense OR MIT | app, cli, worker |
| bytes | 1.12.1 | MIT | app, cli, worker |
| camino | 1.2.6 | MIT OR Apache-2.0 | app |
| cargo-platform | 0.1.9 | MIT OR Apache-2.0 | app |
| cargo_metadata | 0.19.2 | MIT | app |
| cbc | 0.2.1 | MIT OR Apache-2.0 | app, cli, worker |
| census | 0.4.2 | MIT | app, cli |
| cfb | 0.14.0 | MIT | app |
| cfg-if | 1.0.5 | MIT OR Apache-2.0 | app, cli, worker |
| chacha20 | 0.10.2 | MIT OR Apache-2.0 | app, cli, worker |
| chrono | 0.4.45 | MIT OR Apache-2.0 | app, cli, worker |
| cipher | 0.5.2 | MIT OR Apache-2.0 | app, cli, worker |
| clap | 4.6.7 | MIT OR Apache-2.0 | cli |
| clap_builder | 4.6.7 | MIT OR Apache-2.0 | cli |
| clap_derive | 4.6.7 | MIT OR Apache-2.0 | cli |
| clap_lex | 1.1.1 | MIT OR Apache-2.0 | cli |
| cmov | 0.5.4 | Apache-2.0 OR MIT | app, cli |
| cms | 0.2.3 | Apache-2.0 OR MIT | app, cli |
| cms | 0.3.0-pre.1 | Apache-2.0 OR MIT | app, cli |
| color_quant | 1.1.0 | MIT | app, cli, worker |
| colorchoice | 1.0.5 | MIT OR Apache-2.0 | cli |
| concurrent-queue | 2.5.0 | Apache-2.0 OR MIT | app, cli, worker |
| const-oid | 0.10.2 | Apache-2.0 OR MIT | app, cli, worker |
| const-oid | 0.9.6 | Apache-2.0 OR MIT | app, cli |
| cookie | 0.18.2 | MIT OR Apache-2.0 | app |
| core_detect | 1.0.0 | MIT/Apache-2.0 | app, cli, worker |
| core_maths | 0.1.1 | MIT | app, cli |
| cow-utils | 0.1.3 | MIT | app, cli, worker |
| cpubits | 0.1.1 | MIT OR Apache-2.0 | app, cli, worker |
| cpufeatures | 0.2.17 | MIT OR Apache-2.0 | app, cli |
| cpufeatures | 0.3.1 | MIT OR Apache-2.0 | app, cli, worker |
| crc32fast | 1.5.4 | MIT OR Apache-2.0 | app, cli, worker |
| crossbeam-channel | 0.5.17 | MIT OR Apache-2.0 | app, cli |
| crossbeam-deque | 0.8.8 | MIT OR Apache-2.0 | app, cli |
| crossbeam-epoch | 0.9.21 | MIT OR Apache-2.0 | app, cli |
| crossbeam-utils | 0.8.23 | MIT OR Apache-2.0 | app, cli, worker |
| crunchy | 0.2.4 | MIT | app, cli |
| crypto-bigint | 0.5.5 | Apache-2.0 OR MIT | app, cli |
| crypto-common | 0.1.6 | MIT OR Apache-2.0 | app, cli |
| crypto-common | 0.2.2 | MIT OR Apache-2.0 | app, cli, worker |
| cssparser | 0.37.0 | MPL-2.0 | app |
| cssparser-macros | 0.7.1 | MPL-2.0 | app |
| ctor | 1.0.13 | Apache-2.0 OR MIT | app |
| ctr | 0.10.1 | MIT OR Apache-2.0 | app, cli |
| ctutils | 0.4.8 | Apache-2.0 OR MIT | app, cli |
| curve25519-dalek | 4.1.3 | BSD-3-Clause | app |
| curve25519-dalek-derive | 0.1.1 | MIT/Apache-2.0 | app |
| darling | 0.24.1 | MIT | app, cli |
| darling_core | 0.24.1 | MIT | app, cli |
| darling_macro | 0.24.1 | MIT | app, cli |
| dashmap | 6.2.1 | MIT | app, cli, worker |
| data-encoding | 2.11.1 | MIT | app, cli |
| datasketches | 0.5.0 | Apache-2.0 | app, cli |
| der | 0.7.10 | Apache-2.0 OR MIT | app, cli |
| der | 0.8.3 | Apache-2.0 OR MIT | app, cli |
| der-parser | 10.0.0 | MIT OR Apache-2.0 | app, cli |
| der_derive | 0.7.3 | Apache-2.0 OR MIT | app, cli |
| der_derive | 0.8.0 | Apache-2.0 OR MIT | app, cli |
| deranged | 0.5.9 | MIT OR Apache-2.0 | app, cli, worker |
| derive_more | 2.1.1 | MIT | app |
| derive_more-impl | 2.1.1 | MIT | app |
| des | 0.9.0 | MIT OR Apache-2.0 | app, cli |
| digest | 0.10.7 | MIT OR Apache-2.0 | app, cli |
| digest | 0.11.3 | MIT OR Apache-2.0 | app, cli, worker |
| dirs | 7.0.0 | MIT OR Apache-2.0 | app |
| dirs-sys | 0.5.0 | MIT OR Apache-2.0 | app |
| displaydoc | 0.2.7 | MIT OR Apache-2.0 | app, cli, worker |
| dom_query | 0.28.0 | MIT | app |
| downcast-rs | 2.0.2 | MIT OR Apache-2.0 | app, cli |
| dpi | 0.1.2 | Apache-2.0 AND MIT | app |
| dtoa | 1.0.11 | MIT OR Apache-2.0 | app |
| dtoa-short | 0.3.5 | MPL-2.0 | app |
| dunce | 1.0.5 | CC0-1.0 OR MIT-0 OR Apache-2.0 | app |
| dyn-clone | 1.0.20 | MIT OR Apache-2.0 | app |
| dynify | 0.1.2 | MIT OR Apache-2.0 | app, cli, worker |
| dynify-macros | 0.1.2 | MIT OR Apache-2.0 | app, cli, worker |
| ecb | 0.2.1 | MIT OR Apache-2.0 | app, cli, worker |
| ecdsa | 0.16.9 | Apache-2.0 OR MIT | app, cli |
| ed25519 | 2.2.3 | Apache-2.0 OR MIT | app |
| ed25519-dalek | 2.2.0 | BSD-3-Clause | app |
| either | 1.19.0 | MIT OR Apache-2.0 | app, cli, worker |
| elliptic-curve | 0.13.8 | Apache-2.0 OR MIT | app, cli |
| encoding_rs | 0.8.44 | (Apache-2.0 OR MIT) AND BSD-3-Clause | app, cli, worker |
| equator | 0.4.2 | MIT | app, cli, worker |
| equator-macro | 0.4.2 | MIT | app, cli, worker |
| equivalent | 1.0.3 | Apache-2.0 OR MIT | app, cli, worker |
| erased-serde | 0.4.11 | MIT OR Apache-2.0 | app, cli |
| event-listener | 5.4.2 | Apache-2.0 OR MIT | app, cli, worker |
| event-listener-strategy | 0.5.4 | Apache-2.0 OR MIT | app, cli, worker |
| fast-float2 | 0.2.4 | MIT OR Apache-2.0 | app, cli, worker |
| fastdivide | 0.4.2 | zlib-acknowledgement OR MIT | app, cli |
| fastrand | 2.5.0 | Apache-2.0 OR MIT | app, cli, worker |
| fax | 0.2.7 | MIT | app, cli, worker |
| fdeflate | 0.3.7 | MIT OR Apache-2.0 | app, cli, worker |
| ff | 0.13.1 | MIT/Apache-2.0 | app, cli |
| filetime | 0.2.29 | MIT/Apache-2.0 | app |
| fixedbitset | 0.5.7 | MIT OR Apache-2.0 | app, cli, worker |
| flagset | 0.4.7 | Apache-2.0 | app, cli |
| flate2 | 1.1.10 | MIT OR Apache-2.0 | app, cli, worker |
| fnv | 1.0.7 | Apache-2.0 / MIT | app, cli |
| foldhash | 0.2.0 | Zlib | app, cli, worker |
| font-types | 0.11.3 | MIT OR Apache-2.0 | app, cli |
| form_urlencoded | 1.2.2 | MIT OR Apache-2.0 | app |
| fs4 | 0.13.1 | MIT OR Apache-2.0 | app, cli |
| futures-concurrency | 7.7.1 | MIT OR Apache-2.0 | app, cli, worker |
| futures-core | 0.3.34 | MIT OR Apache-2.0 | app, cli, worker |
| futures-io | 0.3.34 | MIT OR Apache-2.0 | app, cli, worker |
| futures-lite | 2.6.1 | Apache-2.0 OR MIT | app, cli, worker |
| futures-macro | 0.3.34 | MIT OR Apache-2.0 | app, cli |
| futures-task | 0.3.34 | MIT OR Apache-2.0 | app, cli |
| futures-util | 0.3.34 | MIT OR Apache-2.0 | app, cli |
| generic-array | 0.14.9 | MIT | app, cli |
| getrandom | 0.2.17 | MIT OR Apache-2.0 | app, cli |
| getrandom | 0.3.4 | MIT OR Apache-2.0 | app, cli |
| getrandom | 0.4.3 | MIT OR Apache-2.0 | app, cli, worker |
| ghash | 0.6.0 | Apache-2.0 OR MIT | app, cli |
| gif | 0.14.2 | MIT OR Apache-2.0 | app, cli, worker |
| glob | 0.3.4 | MIT OR Apache-2.0 | app |
| group | 0.13.0 | MIT/Apache-2.0 | app, cli |
| half | 2.7.1 | MIT OR Apache-2.0 | app, cli, worker |
| hashbrown | 0.12.3 | MIT OR Apache-2.0 | app |
| hashbrown | 0.14.5 | MIT OR Apache-2.0 | app, cli, worker |
| hashbrown | 0.17.1 | MIT OR Apache-2.0 | app, cli, worker |
| heck | 0.5.0 | MIT OR Apache-2.0 | app, cli, worker |
| hex | 0.4.3 | MIT OR Apache-2.0 | app, cli |
| hkdf | 0.12.4 | MIT OR Apache-2.0 | app, cli |
| hmac | 0.12.1 | MIT OR Apache-2.0 | app, cli |
| hmac | 0.13.0 | MIT OR Apache-2.0 | app, cli |
| html5ever | 0.39.0 | MIT OR Apache-2.0 | app |
| htmlescape | 0.3.1 | Apache-2.0 / MIT / MPL-2.0 | app, cli |
| http | 1.5.0 | MIT OR Apache-2.0 | app, cli |
| httparse | 1.10.1 | MIT OR Apache-2.0 | app, cli |
| hybrid-array | 0.4.15 | MIT OR Apache-2.0 | app, cli, worker |
| ico | 0.6.0 | MIT | app |
| icu_collections | 2.3.0 | Unicode-3.0 | app, cli, worker |
| icu_locale_core | 2.4.0 | Unicode-3.0 | app, cli, worker |
| icu_normalizer | 2.3.0 | Unicode-3.0 | app, cli, worker |
| icu_normalizer_data | 2.3.0 | Unicode-3.0 | app, cli, worker |
| icu_properties | 2.3.0 | Unicode-3.0 | app, cli, worker |
| icu_properties_data | 2.3.0 | Unicode-3.0 | app, cli, worker |
| icu_provider | 2.4.0 | Unicode-3.0 | app, cli, worker |
| ident_case | 1.0.1 | MIT/Apache-2.0 | app, cli |
| idna | 1.1.0 | MIT OR Apache-2.0 | app |
| idna_adapter | 1.2.2 | Apache-2.0 OR MIT | app |
| image | 0.25.10 | MIT OR Apache-2.0 | app, cli, worker |
| image-webp | 0.2.4 | MIT OR Apache-2.0 | app, cli, worker |
| indexmap | 1.9.3 | Apache-2.0 OR MIT | app |
| indexmap | 2.14.2 | Apache-2.0 OR MIT | app, cli, worker |
| infer | 0.22.0 | MIT | app |
| inout | 0.2.5 | MIT OR Apache-2.0 | app, cli, worker |
| intrusive-collections | 0.10.3 | MIT OR Apache-2.0 | app, cli, worker |
| inventory | 0.3.25 | MIT OR Apache-2.0 | app, cli |
| is_terminal_polyfill | 1.70.2 | MIT OR Apache-2.0 | cli |
| itertools | 0.14.0 | MIT OR Apache-2.0 | app, cli, worker |
| itertools | 0.15.0 | MIT OR Apache-2.0 | app, cli, worker |
| itoa | 1.0.18 | MIT OR Apache-2.0 | app, cli, worker |
| jpeg-encoder | 0.6.1 | (MIT OR Apache-2.0) AND IJG | app, cli, worker |
| json-patch | 4.2.0 | MIT/Apache-2.0 | app |
| jsonptr | 0.7.1 | MIT OR Apache-2.0 | app |
| keyboard-types | 0.8.3 | MIT OR Apache-2.0 | app |
| kurbo | 0.13.1 | Apache-2.0 OR MIT | app, cli |
| lazy_static | 1.5.1 | MIT OR Apache-2.0 | app, cli |
| levenshtein_automata | 0.2.1 | MIT | app, cli |
| libc | 0.2.191 | MIT OR Apache-2.0 | app |
| libloading | 0.8.9 | ISC | app, cli, worker |
| libm | 0.2.16 | MIT | app, cli |
| litemap | 0.8.3 | Unicode-3.0 | app, cli, worker |
| lock_api | 0.4.14 | MIT OR Apache-2.0 | app, cli, worker |
| log | 0.4.35 | MIT OR Apache-2.0 | app, cli, worker |
| lopdf | 0.46.0 | MIT | app, cli, worker |
| lru | 0.18.5 | MIT | app, cli |
| markup5ever | 0.39.0 | MIT OR Apache-2.0 | app |
| maybe-owned | 0.3.4 | MIT OR Apache-2.0 | app, cli, worker |
| md-5 | 0.11.0 | MIT OR Apache-2.0 | app, cli, worker |
| measure_time | 0.9.0 | MIT | app, cli |
| memchr | 2.8.3 | Unlicense OR MIT | app, cli, worker |
| memmap2 | 0.9.12 | MIT OR Apache-2.0 | app, cli |
| mime | 0.3.17 | MIT OR Apache-2.0 | app |
| minimal-lexical | 0.2.1 | MIT/Apache-2.0 | app, cli |
| miniz_oxide | 0.8.9 | MIT OR Zlib OR Apache-2.0 | app, cli, worker |
| miniz_oxide | 0.9.1 | MIT OR Zlib OR Apache-2.0 | app, cli, worker |
| mio | 1.2.4 | MIT | app |
| moxcms | 0.8.1 | BSD-3-Clause OR Apache-2.0 | app, cli, worker |
| muda | 0.20.0 | Apache-2.0 OR MIT | app |
| multiversion_no_op | 1.0.0 | Apache-2.0 OR MIT | app, cli, worker |
| murmurhash32 | 0.4.0 | MIT | app, cli |
| native-tls | 0.2.18 | MIT OR Apache-2.0 | app, cli |
| new_debug_unreachable | 1.0.6 | MIT | app |
| nom | 7.1.3 | MIT | app, cli |
| nom | 8.0.0 | MIT | app, cli, worker |
| num-bigint | 0.4.8 | MIT OR Apache-2.0 | app, cli |
| num-bigint | 0.5.1 | MIT OR Apache-2.0 | app, cli, worker |
| num-bigint-dig | 0.8.6 | MIT/Apache-2.0 | app, cli |
| num-conv | 0.2.2 | MIT OR Apache-2.0 | app, cli, worker |
| num-integer | 0.1.47 | MIT OR Apache-2.0 | app, cli, worker |
| num-iter | 0.1.46 | MIT OR Apache-2.0 | app, cli |
| num-traits | 0.2.19 | MIT OR Apache-2.0 | app, cli, worker |
| num_enum | 0.7.6 | BSD-3-Clause OR MIT OR Apache-2.0 | app, cli, worker |
| num_enum_derive | 0.7.6 | BSD-3-Clause OR MIT OR Apache-2.0 | app, cli, worker |
| oid-registry | 0.8.1 | MIT OR Apache-2.0 | app, cli |
| once_cell | 1.21.4 | MIT OR Apache-2.0 | app, cli, worker |
| once_cell_polyfill | 1.70.2 | MIT OR Apache-2.0 | cli |
| oneshot | 0.1.13 | MIT OR Apache-2.0 | app, cli |
| oneshot | 0.2.1 | MIT OR Apache-2.0 | app, cli, worker |
| option-ext | 0.2.0 | MPL-2.0 | app |
| ordered-float | 5.5.1 | MIT | app, cli |
| ownedbytes | 0.9.0 | MIT | app, cli |
| p12-keystore | 0.4.1 | MIT/Apache-2.0 | app, cli |
| p256 | 0.13.2 | Apache-2.0 OR MIT | app, cli |
| p384 | 0.13.1 | Apache-2.0 OR MIT | app, cli |
| parking | 2.2.1 | Apache-2.0 OR MIT | app, cli, worker |
| parking_lot | 0.12.5 | MIT OR Apache-2.0 | app |
| parking_lot_core | 0.9.12 | MIT OR Apache-2.0 | app, cli, worker |
| pastey | 0.2.3 | MIT OR Apache-2.0 | app, cli, worker |
| pbkdf2 | 0.13.0 | MIT OR Apache-2.0 | app, cli |
| pdfium-render | 0.9.5 | MIT OR Apache-2.0 | app, cli, worker |
| pem-rfc7468 | 0.7.0 | Apache-2.0 OR MIT | app, cli |
| pem-rfc7468 | 1.0.0 | Apache-2.0 OR MIT | app, cli |
| percent-encoding | 2.3.2 | MIT OR Apache-2.0 | app, cli |
| phf | 0.13.1 | MIT | app |
| phf | 0.14.0 | MIT | app, cli, worker |
| phf_generator | 0.13.1 | MIT | app |
| phf_generator | 0.14.0 | MIT | app, cli, worker |
| phf_macros | 0.13.1 | MIT | app |
| phf_macros | 0.14.0 | MIT | app, cli, worker |
| phf_shared | 0.13.1 | MIT | app |
| phf_shared | 0.14.0 | MIT | app, cli, worker |
| pin-project | 1.1.13 | Apache-2.0 OR MIT | app, cli, worker |
| pin-project-internal | 1.1.13 | Apache-2.0 OR MIT | app, cli, worker |
| pin-project-lite | 0.2.17 | Apache-2.0 OR MIT | app, cli, worker |
| piston-float | 1.0.1 | MIT | app, cli, worker |
| pkcs1 | 0.7.5 | Apache-2.0 OR MIT | app, cli |
| pkcs12 | 0.2.0-pre.0 | Apache-2.0 OR MIT | app, cli |
| pkcs5 | 0.8.1 | Apache-2.0 OR MIT | app, cli |
| pkcs8 | 0.10.2 | Apache-2.0 OR MIT | app, cli |
| pkcs8 | 0.11.0 | Apache-2.0 OR MIT | app, cli |
| plist | 1.10.3 | MIT | app |
| png | 0.17.16 | MIT OR Apache-2.0 | app |
| png | 0.18.1 | MIT OR Apache-2.0 | app, cli, worker |
| polycool | 0.4.0 | MIT OR Apache-2.0 | app, cli |
| polyval | 0.7.3 | Apache-2.0 OR MIT | app, cli |
| portable-atomic | 1.15.0 | Apache-2.0 OR MIT | app, cli, worker |
| potential_utf | 0.1.6 | Unicode-3.0 | app, cli, worker |
| powerfmt | 0.2.1 | MIT OR Apache-2.0 | app, cli, worker |
| ppv-lite86 | 0.2.21 | MIT OR Apache-2.0 | app, cli |
| precomputed-hash | 0.1.1 | MIT | app |
| prettyplease | 0.3.0 | MIT OR Apache-2.0 | app, cli |
| primeorder | 0.13.6 | Apache-2.0 OR MIT | app, cli |
| proc-macro-crate | 3.5.0 | MIT OR Apache-2.0 | app, cli, worker |
| proc-macro2 | 1.0.107 | MIT OR Apache-2.0 | app, cli, worker |
| pulldown-cmark | 0.13.6 | MIT | app, cli |
| pxfm | 0.1.30 | BSD-3-Clause OR Apache-2.0 | app, cli, worker |
| quick-error | 2.0.1 | MIT/Apache-2.0 | app, cli, worker |
| quick-xml | 0.42.0 | MIT | app |
| quote | 1.0.47 | MIT OR Apache-2.0 | app, cli, worker |
| rand | 0.10.3 | MIT OR Apache-2.0 | app, cli, worker |
| rand | 0.8.8 | MIT OR Apache-2.0 | app, cli |
| rand | 0.9.5 | MIT OR Apache-2.0 | app, cli |
| rand_chacha | 0.3.1 | MIT OR Apache-2.0 | app, cli |
| rand_chacha | 0.9.0 | MIT OR Apache-2.0 | app, cli |
| rand_core | 0.10.1 | MIT OR Apache-2.0 | app, cli, worker |
| rand_core | 0.6.4 | MIT OR Apache-2.0 | app, cli |
| rand_core | 0.9.5 | MIT OR Apache-2.0 | app, cli |
| rangemap | 1.8.0 | MIT/Apache-2.0 | app, cli, worker |
| raw-window-handle | 0.6.2 | MIT OR Apache-2.0 OR Zlib | app |
| rayon | 1.12.0 | MIT OR Apache-2.0 | app, cli |
| rayon-core | 1.13.0 | MIT OR Apache-2.0 | app, cli |
| rc2 | 0.9.0 | MIT OR Apache-2.0 | app, cli |
| read-fonts | 0.39.2 | MIT OR Apache-2.0 | app, cli |
| regex | 1.13.1 | MIT OR Apache-2.0 | app, cli |
| regex-automata | 0.4.18 | MIT OR Apache-2.0 | app, cli |
| regex-syntax | 0.8.11 | MIT OR Apache-2.0 | app, cli |
| regress | 0.12.0 | MIT OR Apache-2.0 | app, cli, worker |
| rfc6979 | 0.4.0 | Apache-2.0 OR MIT | app, cli |
| rfd | 0.17.2 | MIT | app |
| rsa | 0.9.10 | MIT OR Apache-2.0 | app, cli |
| rustc-hash | 2.1.3 | Apache-2.0 OR MIT | app, cli, worker |
| rusticata-macros | 4.1.0 | MIT/Apache-2.0 | app, cli |
| rustls-pki-types | 1.15.1 | MIT OR Apache-2.0 | app, cli |
| rustversion | 1.0.23 | MIT OR Apache-2.0 | app, cli, worker |
| rustybuzz | 0.20.1 | MIT | app, cli |
| ryu-js | 1.0.3 | Apache-2.0 OR BSL-1.0 | app, cli, worker |
| salsa20 | 0.11.0 | MIT OR Apache-2.0 | app, cli |
| same-file | 1.0.6 | Unlicense/MIT | app |
| schannel | 0.1.30 | MIT | app, cli |
| schemars | 0.8.22 | MIT | app |
| schemars_derive | 0.8.22 | MIT | app |
| scopeguard | 1.2.0 | MIT OR Apache-2.0 | app, cli, worker |
| scrypt | 0.12.0 | MIT OR Apache-2.0 | app, cli |
| sec1 | 0.7.3 | Apache-2.0 OR MIT | app, cli |
| selectors | 0.38.0 | MPL-2.0 | app |
| semver | 1.0.28 | MIT OR Apache-2.0 | app |
| serde | 1.0.229 | MIT OR Apache-2.0 | app, cli, worker |
| serde-tuple-vec-map | 1.0.1 | MIT | app, cli |
| serde-untagged | 0.1.9 | MIT OR Apache-2.0 | app |
| serde_core | 1.0.229 | MIT OR Apache-2.0 | app, cli, worker |
| serde_derive | 1.0.229 | MIT OR Apache-2.0 | app, cli, worker |
| serde_derive_internals | 0.29.1 | MIT OR Apache-2.0 | app |
| serde_json | 1.0.154 | MIT OR Apache-2.0 | app, cli, worker |
| serde_repr | 0.1.21 | MIT OR Apache-2.0 | app |
| serde_spanned | 1.1.2 | MIT OR Apache-2.0 | app, cli |
| serde_with | 3.24.0 | MIT OR Apache-2.0 | app |
| serde_with_macros | 3.24.0 | MIT OR Apache-2.0 | app |
| serialize-to-javascript | 0.1.2 | MIT OR Apache-2.0 | app |
| serialize-to-javascript-impl | 0.1.2 | MIT OR Apache-2.0 | app |
| servo_arc | 0.4.3 | MIT OR Apache-2.0 | app |
| sha1 | 0.10.7 | MIT OR Apache-2.0 | app, cli |
| sha1 | 0.11.0 | MIT OR Apache-2.0 | app, cli |
| sha2 | 0.10.9 | MIT OR Apache-2.0 | app, cli |
| sha2 | 0.11.1 | MIT OR Apache-2.0 | app, cli, worker |
| signature | 2.2.0 | Apache-2.0 OR MIT | app, cli |
| simd-adler32 | 0.3.10 | MIT | app, cli, worker |
| simdutf8 | 0.1.5 | MIT OR Apache-2.0 | app, cli, worker |
| siphasher | 1.0.4 | MIT OR Apache-2.0 | app, cli, worker |
| sketches-ddsketch | 0.4.2 | Apache-2.0 | app, cli |
| skrifa | 0.42.1 | MIT OR Apache-2.0 | app, cli |
| slab | 0.4.12 | MIT | app, cli |
| small_btree | 0.1.0 | Unlicense OR MIT | app, cli, worker |
| smallvec | 1.16.4 | MIT OR Apache-2.0 | app, cli, worker |
| socket2 | 0.6.5 | MIT OR Apache-2.0 | app |
| softbuffer | 0.4.8 | MIT OR Apache-2.0 | app |
| spin | 0.9.9 | MIT | app, cli |
| spki | 0.7.3 | Apache-2.0 OR MIT | app, cli |
| spki | 0.8.1 | Apache-2.0 OR MIT | app, cli |
| stable_deref_trait | 1.2.1 | MIT OR Apache-2.0 | app, cli, worker |
| static_assertions | 1.1.0 | MIT OR Apache-2.0 | app, cli, worker |
| string_cache | 0.9.0 | MIT OR Apache-2.0 | app |
| stringprep | 0.1.5 | MIT/Apache-2.0 | app, cli, worker |
| strsim | 0.11.1 | MIT | app, cli |
| strum | 0.28.0 | MIT | app, cli, worker |
| strum_macros | 0.28.0 | MIT | app, cli, worker |
| subsetter | 0.2.6 | MIT OR Apache-2.0 | app, cli |
| subtle | 2.6.1 | BSD-3-Clause | app, cli |
| syn | 2.0.119 | MIT OR Apache-2.0 | app, cli, worker |
| syn | 3.0.9 | MIT OR Apache-2.0 | app, cli, worker |
| synstructure | 0.13.2 | MIT | app, cli |
| synstructure | 0.14.0 | MIT | app, cli, worker |
| tag_ptr | 0.1.0 | Unlicense OR MIT | app, cli, worker |
| tantivy | 0.27.0 | MIT | app, cli |
| tantivy-bitpacker | 0.27.0 | MIT | app, cli |
| tantivy-columnar | 0.27.0 | MIT | app, cli |
| tantivy-common | 0.27.0 | MIT | app, cli |
| tantivy-fst | 0.5.0 | Unlicense/MIT | app, cli |
| tantivy-query-grammar | 0.27.0 | MIT | app, cli |
| tantivy-sstable | 0.27.0 | MIT | app, cli |
| tantivy-stacker | 0.27.0 | MIT | app, cli |
| tantivy-tokenizer-api | 0.7.0 | MIT | app, cli |
| tao | 0.37.2 | Apache-2.0 | app |
| tap | 1.0.1 | MIT | app, cli, worker |
| tar | 0.4.46 | MIT OR Apache-2.0 | app |
| tauri | 2.12.5 | Apache-2.0 OR MIT | app |
| tauri-codegen | 2.7.4 | Apache-2.0 OR MIT | app |
| tauri-macros | 2.7.4 | Apache-2.0 OR MIT | app |
| tauri-plugin-dialog | 2.8.4 | Apache-2.0 OR MIT | app |
| tauri-plugin-fs | 2.7.1 | Apache-2.0 OR MIT | app |
| tauri-plugin-single-instance | 2.5.5 | Apache-2.0 OR MIT | app |
| tauri-runtime | 2.12.2 | Apache-2.0 OR MIT | app |
| tauri-runtime-wry | 2.12.3 | Apache-2.0 OR MIT | app |
| tauri-utils | 2.10.2 | Apache-2.0 OR MIT | app |
| tempfile | 3.27.0 | MIT OR Apache-2.0 | app, cli |
| tendril | 0.5.1 | MIT OR Apache-2.0 | app |
| thin-vec | 0.2.21 | MIT OR Apache-2.0 | app, cli, worker |
| thiserror | 2.0.21 | MIT OR Apache-2.0 | app, cli, worker |
| thiserror-impl | 2.0.21 | MIT OR Apache-2.0 | app, cli, worker |
| tiff | 0.11.3 | MIT | app, cli, worker |
| time | 0.3.55 | MIT OR Apache-2.0 | app, cli, worker |
| time-core | 0.1.9 | MIT OR Apache-2.0 | app, cli, worker |
| time-macros | 0.2.32 | MIT OR Apache-2.0 | app, cli, worker |
| tinystr | 0.8.5 | Unicode-3.0 | app, cli, worker |
| tinyvec | 1.13.3 | Zlib OR Apache-2.0 OR MIT | app, cli, worker |
| tokio | 1.54.1 | MIT | app |
| toml | 1.1.8+spec-1.1.0 | MIT OR Apache-2.0 | app, cli |
| toml_datetime | 1.1.2+spec-1.1.0 | MIT OR Apache-2.0 | app, cli, worker |
| toml_edit | 0.25.17+spec-1.1.0 | MIT OR Apache-2.0 | app, cli, worker |
| toml_parser | 1.1.5+spec-1.1.0 | MIT OR Apache-2.0 | app, cli, worker |
| toml_writer | 1.1.3+spec-1.1.0 | MIT OR Apache-2.0 | app, cli |
| tracing | 0.1.44 | MIT | app |
| tracing-attributes | 0.1.31 | MIT | app |
| tracing-core | 0.1.36 | MIT | app |
| ttf-parser | 0.25.1 | MIT OR Apache-2.0 | app, cli, worker |
| typeid | 1.0.3 | MIT OR Apache-2.0 | app, cli |
| typenum | 1.20.1 | MIT OR Apache-2.0 | app, cli, worker |
| typetag | 0.2.23 | MIT OR Apache-2.0 | app, cli |
| typetag-impl | 0.2.23 | MIT OR Apache-2.0 | app, cli |
| unicase | 2.10.0 | MIT OR Apache-2.0 | app, cli |
| unicode-bidi | 0.3.18 | MIT OR Apache-2.0 | app, cli, worker |
| unicode-bidi-mirroring | 0.4.0 | MIT/Apache-2.0 | app, cli |
| unicode-ccc | 0.4.0 | MIT/Apache-2.0 | app, cli |
| unicode-ident | 1.0.27 | (MIT OR Apache-2.0) AND Unicode-3.0 | app, cli, worker |
| unicode-normalization | 0.1.26 | (MIT OR Apache-2.0) AND Unicode-3.0 | app, cli, worker |
| unicode-properties | 0.1.5 | (MIT OR Apache-2.0) AND Unicode-3.0 | app, cli, worker |
| unicode-script | 0.5.9 | MIT OR Apache-2.0 | app, cli |
| unicode-segmentation | 1.13.3 | MIT OR Apache-2.0 | app |
| universal-hash | 0.6.1 | MIT OR Apache-2.0 | app, cli |
| unwrap-infallible | 1.0.0 | MIT OR Apache-2.0 | app, cli |
| ureq | 3.5.1 | MIT OR Apache-2.0 | app, cli |
| ureq-proto | 0.7.1 | MIT OR Apache-2.0 | app, cli |
| url | 2.5.8 | MIT OR Apache-2.0 | app |
| urlpattern | 0.6.0 | MIT | app |
| utf16_iter | 1.1.0 | Apache-2.0 OR MIT | app, cli, worker |
| utf16string | 0.2.0 | MIT OR Apache-2.0 | app, cli, worker |
| utf8-ranges | 1.0.5 | Unlicense/MIT | app, cli |
| utf8-zero | 0.8.1 | MIT OR Apache-2.0 | app, cli |
| utf8_iter | 1.1.0 | Apache-2.0 OR MIT | app, cli, worker |
| utf8parse | 0.2.2 | Apache-2.0 OR MIT | cli |
| utf_types | 1.0.0 | Apache-2.0 OR MIT | app, cli, worker |
| uuid | 1.29.0 | Apache-2.0 OR MIT | app, cli |
| vecmath | 1.0.0 | MIT | app, cli, worker |
| walkdir | 2.5.0 | Unlicense/MIT | app |
| web-time | 1.1.0 | MIT OR Apache-2.0 | app |
| web_atoms | 0.2.6 | MIT OR Apache-2.0 | app |
| webview2-com | 0.39.1 | MIT | app |
| webview2-com-macros | 0.8.1 | MIT | app |
| webview2-com-sys | 0.39.1 | MIT | app |
| weezl | 0.1.12 | MIT OR Apache-2.0 | app, cli, worker |
| weezl | 0.2.1 | MIT OR Apache-2.0 | app, cli, worker |
| winapi | 0.3.9 | MIT/Apache-2.0 | app, cli |
| winapi-util | 0.1.11 | Unlicense OR MIT | app |
| window-vibrancy | 0.8.1 | Apache-2.0 OR MIT | app |
| windows | 0.62.2 | MIT OR Apache-2.0 | app |
| windows-collections | 0.3.2 | MIT OR Apache-2.0 | app |
| windows-core | 0.62.2 | MIT OR Apache-2.0 | app |
| windows-future | 0.3.2 | MIT OR Apache-2.0 | app |
| windows-implement | 0.60.2 | MIT OR Apache-2.0 | app |
| windows-interface | 0.59.3 | MIT OR Apache-2.0 | app |
| windows-link | 0.2.1 | MIT OR Apache-2.0 | app, cli, worker |
| windows-numerics | 0.3.1 | MIT OR Apache-2.0 | app |
| windows-result | 0.4.1 | MIT OR Apache-2.0 | app |
| windows-strings | 0.5.1 | MIT OR Apache-2.0 | app |
| windows-sys | 0.59.0 | MIT OR Apache-2.0 | app, cli |
| windows-sys | 0.61.2 | MIT OR Apache-2.0 | app, cli, worker |
| windows-targets | 0.52.6 | MIT OR Apache-2.0 | app, cli |
| windows-threading | 0.2.1 | MIT OR Apache-2.0 | app |
| windows-version | 0.1.7 | MIT OR Apache-2.0 | app |
| windows_x86_64_msvc | 0.52.6 | MIT OR Apache-2.0 | app, cli |
| winnow | 1.0.4 | MIT | app, cli, worker |
| winreg | 0.56.0 | MIT | app, cli |
| write-fonts | 0.48.1 | MIT OR Apache-2.0 | app, cli |
| write16 | 1.0.0 | Apache-2.0 OR MIT | app, cli, worker |
| writeable | 0.6.4 | Unicode-3.0 | app, cli, worker |
| wry | 0.57.1 | Apache-2.0 OR MIT | app |
| x509-cert | 0.2.5 | Apache-2.0 OR MIT | app, cli |
| x509-cert | 0.3.0 | Apache-2.0 OR MIT | app, cli |
| x509-parser | 0.18.1 | MIT OR Apache-2.0 | app, cli |
| yoke | 0.8.4 | Unicode-3.0 | app, cli, worker |
| yoke-derive | 0.8.4 | Unicode-3.0 | app, cli, worker |
| zerocopy | 0.8.63 | BSD-2-Clause OR Apache-2.0 OR MIT | app, cli, worker |
| zerocopy-derive | 0.8.63 | BSD-2-Clause OR Apache-2.0 OR MIT | app, cli, worker |
| zerofrom | 0.1.8 | Unicode-3.0 | app, cli, worker |
| zerofrom-derive | 0.1.8 | Unicode-3.0 | app, cli, worker |
| zeroize | 1.9.1 | Apache-2.0 OR MIT | app, cli |
| zerotrie | 0.2.5 | Unicode-3.0 | app, cli, worker |
| zerovec | 0.11.9 | Unicode-3.0 | app, cli, worker |
| zerovec-derive | 0.11.7 | Unicode-3.0 | app, cli, worker |
| zip | 2.6.1 | MIT | app, cli |
| zmij | 1.0.23 | MIT | app, cli, worker |
| zopfli | 0.8.4 | Apache-2.0 | app, cli |
| zstd | 0.13.4 | BSD-3-Clause | app |
| zstd-safe | 7.4.0 | BSD-3-Clause | app |
| zstd-sys | 2.1.2+zstd.1.5.7 | BSD-3-Clause | app |
| zune-core | 0.5.3 | MIT OR Apache-2.0 OR Zlib | app, cli, worker |
| zune-jpeg | 0.5.15 | MIT OR Apache-2.0 OR Zlib | app, cli, worker |
