---
title: Beta wgsl-rs on crates.io
date: 2026-10-01
---

_wgsl-rs beta release retro_

# wgsl-rs is released!

Wow! So much work went into this beta release. It's been the most organized project I've ever attempted,
and I think that shows in the [beta project board](https://github.com/users/schell/projects/3).

If you're interested in _what_ `wgsl-rs` is, see the github repo [https://github.com/schell/wgsl-rs](https://github.com/schell/wgsl-rs),
or the previous article [Introducing `wgsl-rs`](/articles/introducing-wgsl-rs.html).

## Life events

Looking back, `wgsl-rs` was definitely a bite too big for me to chew as a single developer working
part time. **But luckily, I lost my day job early September** and found myself with all the time in
the world to work on the project 😉.

<small>(I didn't really have **all the time**, but I had more than usual)</small>

## Highlights

In the four months since [Introducing `wgsl-rs`](/articles/introducing-wgsl-rs.html), the work has primarily been about three
things:

1. Pivoting `wgpu` codegen onto the runtime IR
2. Systematically closing every place Rust and WGSL silently disagree
3. Actually _shipping_ the `0.1.0-beta` (which consisted of ensuring I covered the WGSL vocab, fixed bugs and had documentation in place)

I went digging through the devlog to find the most interesting-for-a-technical-audience developments.

### The top 5 most interesting things, I think 

1. **The operator precedence flip ([#159](https://github.com/schell/wgsl-rs/issues/159), Sept 20).**
   - *Problem*: Rust binds `& ^ | << >>` tighter than `==`; WGSL binds the reverse. The renderer emitted binary expressions flat, so `(0 & 0) == 0` in Rust became `0 & 0 == 0` in WGSL, which re-parses as `0 & (0 == 0)` — a silent type-error miscompile (same class broke `-(a + b)` into `-a + b`).
   - *Fix*: fully parenthesize every binary expression, deliberately choosing "correctness over output polish" over a precedence-aware renderer.

2. **Vector equality is a mask ([#164](https://github.com/schell/wgsl-rs/issues/164), Sept 15).**
   - *Problem*: Rust `==` yields `bool`; WGSL `==` yields `vecN<bool>`.
   - *Fix*: Rust code could never produce the componentwise mask, so wgsl-rs added a post-monomorphization pass rewriting vector `==` to `all(lhs == rhs)` (and `!=` to `!(all(...))`), plus `cmp_eq`/`cmp_ne` free functions as the componentwise escape hatch. The pass fails open on un-inferable types so CPU-only code keeps compiling.

3. **The literal-suffix pass ([#145](https://github.com/schell/wgsl-rs/issues/145), Sept 21–26).**
   - *Problem*: Rust infers `0` as `u32` from context; WGSL defaults bare ints to `i32`, so `select(0, 1, data)` compiled in Rust and failed `naga` validation.
   - *Fix*: I (and GLM 5.2) built a scope-tracking "anchoring pass" placed at the deshadow slot so it sees all four sources of literals (source, const substitution, type substitution, extension lowering).
    After seven review rounds with Copilot and Kimi K3 it was consolidated into a bottom-up `infer(expr) -> Option<Type>` with an exhaustive match and no catch-all.
    Loop-variable typing mirrors rustc's backward inference via a "probe, not a constraint solver."

4. **Runtime wgpu linkage from the IR (June 6, [#120](https://github.com/schell/wgsl-rs/issues/120), plus July 11/18).**
   - ~650 lines of proc-macro-generated `wgpu` codegen (walking the `syn` tree) were replaced by runtime IR traversal.
   This unlocked a rad feature: generic "*template*" modules can now produce real `wgpu` pipelines after calling `path::to::module::instantiate::<A, B, C>()` at **runtime**.
   Buffer sizing follows WGSL §14.4.1 instead of `size_of::<T>()`, which was wrong for non-`repr(C)` structs.
   The API split is a nice design story: `wgsl_rs::Source` is "the spec," `ir::Module` is "the AST," and methods live in extension traits because `wgsl-rs-ir`
   can't depend on `wgsl-rs` without a cycle.

5. **Binding stage visibility from the call graph (July 18, then [#177](https://github.com/schell/wgsl-rs/issues/177), Sept 20).**
   - *Problem*: The analyzer hardcoded `ShaderStages::all()`, which silently demanded `VERTEX_WRITABLE_STORAGE` and broke every `read_write`
     compute dispatch.
   - *Fix*: Two iterations: first per-entry-point identifier scanning, then full transitive call-graph reachability,
     so bindings used only in helper functions resolve correctly.
     A whole-module scan was rejected because over-broad visibility forces features on users who don't want them.

## Users

Part of the impetus to jam on the beta release is that the project already has users!
There's two users in particular that reached out to me. The first is Jak Kos (jakkos-net on github).

> Hiya, super cool project, I've been having fun the last few days porting the wgsl shaders of my gamedev project over, so thank you :D !
> -- jakkos-net

They identified a bunch of gaps in my initial build out like missing support for storage textures.
[They've been really helpful, creating tickets for bugs](https://github.com/schell/wgsl-rs/issues?q=is%3Aissue+state%3Aclosed+author%3Ajakkos-net)
they've found while they port their game to `wgsl-rs`.

Thank you Jak! 🙇

### gpui-ce

The other exciting development is that the [gpui-ce](https://github.com/gpui-ce/gpui-ce/) project is using `wgsl-rs` for their shader layer.

[gpui-ce](https://github.com/gpui-ce/gpui-ce/) is a community fork of the Zed editor's `gpui` crate.
It offers an immediate-mode rendering API (though it's more nuanced than that) inspired by the web DOM.

It's a big impressive project with currently 1.2k stars and I'm honored that they've integrated my project.

Funnily enough, I almost missed learning about this since my email client put Miles Wirht's introductory email in the spam folder!
Miles is a maintainer of the project and reached out to say hi and show me the shaders in their repo.

Thanks Miles!

### Others?

If you're using `wgsl-rs` and would like a mention, please reach out!

## That's a wrap 🌯

My next focus is going to be on porting [crabslab](https://crates.io/crates/crabslab) and [craballoc](https://crates.io/crates/craballoc) from Rust-GPU to `wgsl-rs`.
After that I'll be writing an ECS that runs on the GPU and then using this whole stack to rewrite the shaders and linkage of Renderling.

Thanks for reading :)
