---
title: Introducing wgsl-rs
date: Thu 05 Mar, 2026
---

_Wherein I introduce a new WGSL library_

<!-- 

TODO: Implement the table of contents

My private stuff used for editing. 
Pay no attention to the man behind the curtain.

👍🤞🍖🚧🔗🤦🙇☕

...⏱️

😭😈😉😊

🇳🇿 🏝️

<video controls width="100%">
  <source src="" type="video/mp4" />
  Backup text.
</video>


NOTE: THERE MUST NOT BE EMPTY LINES

<div class="images-horizontal">
    <div class="image">
        <label>Label</label>
        <img class="pixelated" width="100" src="" />
    </div>
    <div class="image">
        <label>Label</label>
        <img class="pixelated" width="100" src="" />
    </div>
</div>

<div class="image">
    <label>Label</label>
    <img
        src=""
        alt="" />
</div>
-->

# Rust as a shader language 

If you've read this devlog before then you already know that Renderling is a real-time 3d renderer built with Rust.
One thing that sets it apart from other real-time renderers is the fact that all the shaders are also written
in Rust, which is unusual. The underlying tech that enables this is [Rust-GPU](https://github.com/rust-gpu/rust-gpu).
`Rust-GPU` is a `rustc` compiler backend that does code generation to produce SPIR-V from Rust. This means your Rust
code can run anywhere SPIR-V runs, which is pretty much every GPU. `Rust-GPU` is awesome.

This has a lot of obvious benefits. Rust as a shader language is great.

1. You get full use of Rust's module system.

2. Type checking.

3. Expressions.

4. You get generics, traits, borrowing, etc.

5. Editor tooling (LSP, etc.).

6. `cargo`, crates.io, docs.rs, etc.

But you don't get everything. You don't get allocation.
Essentially you're writing `#[no_std]` Rust.
That's fine though, there's _a lot_ you can do without dynamically allocating.
The `Rust-GPU` team is hard at work to enable dynamic allocation, though, and that will be very interesting once it's
up and running.

There are also some detriments, but they come more from the implementation of the `Rust-GPU` system than from Rust itself.

1. Provisioning the compiler backend is a hassle.

   I personally put a lot of work into [`cargo-gpu`](https://github.com/rust-gpu/cargo-gpu),
   a cargo plugin that wrangles a lot of that complexity for you. This tool gets you really far in terms of provisioning,
   but it's still quite a large step to wield it correctly.

2. Your shader code is tied to a specific Rust compiler, and a specific version of the GPU "std" library [`spirv-std`](https://crates.io/crates/spirv-std).

   This has been a tough one for me, because my grant project work depends on me being able to release crates, but `spirv-std` hasn't been updated
   for over two years. I basically failed as a maintainer of `Rust-GPU` to get a regular release cadence going. The team needed more compiler-level
   developers to get upstream `rustc` changes through the pipe and I couldn't really contribute there. In the grand scheme of things I had to decide
   whether I would spend time becoming a compiler dev or spend time working on my rendering library. I've chosen rendering because it sparks joy for
   me.

3. You have two different compilation scenarios.

   There's your regular CPU Rust code to compile, and then there's your GPU Rust code (shaders) that compile to SPIR-V.
  The SPIR-V binaries then need to be linked back to your CPU code, and how you do this is up to you. 

4. The Rust you write is actually a _subset_ of `#[no_std]` Rust code.

   * Can't use an array as a slice, it causes this error:
     ```
     error: cannot cast between pointer types
            from `*[u32; 3]`
              to `*[u32]`  
     ```
     See the ticket I opened [Rust-GPU/rust-gpu#465](https://github.com/rust-gpu/rust-gpu#465).
   * You can use enums but they must be simple (like #[repr(u32)])
   * Don't use `while let` or `while` loops
   * `for` loops are hit or miss, see [Rust-GPU/rust-gpu#123](https://github.com/Rust-GPU/rust-gpu/issues/123).
   * No `transmute` or unsafe pointer casts.

     Any function that uses `core::mem::transmute` or other forms of type punning fails to compile.
     This one can be tricky because the error message is not totally helpful, and you have to dive into the function call
     graph to figure out where the problem is, then find a replacement.

   I have a list of pro/cons and gotchas [here](https://github.com/schell/renderling/blob/main/NOTES.md#cons--limititions--gotchas) if you want to
   look a little deeper.

   But the kicker here is that you don't know about a gotcha until you hit it.
   I'm sure there are issues I'm not tracking that I simply haven't hit.

## Rust-GPU to Vectorware

Lastly, the rest of the Rust-GPU maintainers have incorporated as "vectorware" and are working on their product, and the communication (at least with me) has kinda
dried up.
It makes sense.
Startups are a lot of work.
I took this as a sign to get my priorities straight and made a decision to rely on my own tech stack.
The other maintainers are great.
They're awesome engineers and I wish them the best in their corporate endeavor.
I however, am on a quest for graphics and sound and games, and they are on a quest for AI and general compute and startups, so we are walking our own paths.
Our paths will likely cross often!

...but they're separate.
   
## Worse is better?

Given Rust-GPU's strengths and limitations, I decided to write a crate that could give me 80% of the benefit that Rust-GPU provides me, with
20% of the effort.

(It's probably more like 10% or 5% - these numbers are squishy and hand-wavey).

That crate is called [`wgsl-rs`] and it is a procedural macro crate that lets you write WGSL shaders in a subset of Rust.
You annotate a Rust module with `#[wgsl]` and the macro does two things:
it leaves your Rust code intact (so it compiles and runs on the CPU as normal), and it transpiles that same code
into WGSL for the GPU. Same types, same logic, two targets.

It works on stable Rust (and nightly).
No custom compiler backend.
No nightly toolchain.
No SPIR-V step.
You `cargo add wgsl-rs` and you're off.

The generated WGSL is human-readable!
It looks almost exactly like the Rust you wrote, just in WGSL syntax.
And if you enable the `linkage-wgpu` feature, `wgsl-rs` also generates all the `wgpu` boilerplate for you: buffer
descriptors, bind group layouts, shader module creation, and entry point helpers. That's a _lot_ of tedious code you
don't have to write by hand.

## How it works

You write a Rust module and put `#[wgsl]` on it. Inside, you use types from `wgsl_rs::std` and annotate your
entry points with `#[vertex]`, `#[fragment]`, or `#[compute]`. Here's a complete hello-world shader rendering
a triangle that changes color over time.
This is a port of the shader from [Tour of WGSL](https://google.github.io/tour-of-wgsl/):

```rust
use wgsl_rs::wgsl;

#[wgsl]
pub mod hello_triangle {
    use wgsl_rs::std::*;

    uniform!(group(0), binding(0), FRAME: u32);

    #[vertex]
    pub fn vtx_main(#[builtin(vertex_index)] vertex_index: u32) -> Vec4f {
        const POS: [Vec2f; 3] = [
            vec2f(0.0, 0.5),
            vec2f(-0.5, -0.5),
            vec2f(0.5, -0.5),
        ];

        let position = POS[vertex_index as usize];
        vec4f(position.x, position.y, 0.0, 1.0)
    }

    #[fragment]
    pub fn frag_main() -> Vec4f {
        vec4f(1.0, sin(f32(get!(FRAME)) / 128.0), 0.0, 1.0)
    }
}
```

That's real Rust code! You can call `hello_triangle::vtx_main(0)` on the CPU and get a `Vec4f` back.
You can write unit tests against your shader logic. You can step through it in a debugger, though I
hardly ever use debuggers with Rust - but you can!

And here's the WGSL that `wgsl-rs` generates from that same code:

```wgsl
@binding(0) @group(0) var<uniform> FRAME : u32;

@vertex
fn vtx_main(@builtin(vertex_index) vertex_index : u32) -> @builtin(position) vec4f {
    const POS = array(
        vec2f(0.0, 0.5),
        vec2f(-0.5, -0.5),
        vec2f(0.5, -0.5)
    );

    let position = POS[vertex_index];
    return vec4f(position.x, position.y, 0.0, 1.0);
}

@fragment
fn frag_main() -> @location(0) vec4f {
    return vec4f(1.0, sin(f32(FRAME) / 128.0), 0.0, 1.0);
}
```

Notice how close the two are. The structure is preserved, the names are preserved, and you can read the WGSL output
without needing a decoder ring. If something goes wrong on the GPU, you can look at the generated WGSL and reason
about it.

The `uniform!` macro declares a uniform binding in both Rust and WGSL simultaneously. On the Rust side it creates a
thread-safe module-level variable; on the WGSL side it emits the `@binding` / `@group` / `var<uniform>` declaration.
The `get!` macro reads that variable in a way that works on both targets.
Accessing uniforms and storage is a bit of a wart in that I had to use macros, but it is what it is.

Standalone modules (those without imports from other `#[wgsl]` modules) are validated at compile time using
[naga](https://github.com/gfx-rs/wgpu/tree/trunk/naga), and validation errors are mapped back to your Rust source
spans. So if you write invalid WGSL, you see the error in your editor, on the right line, before you ever run
the shader.
That feature isn't perfect, more work could be done to make the spans more accurate, but it's pretty good.

If a module _does_ import from another `#[wgsl]` module, the full concatenated WGSL can't be assembled during macro
expansion, so `wgsl-rs` generates a `#[test]` function that validates it instead. Run `cargo test` and you know your
shaders are valid.

## What you get

Here I'll compare the Rust-GPU pain points from earlier.

1. **No provisioning headaches.**

   There's no compiler backend to install. No `cargo-gpu`, no custom `rustup` components.
   It's `cargo add wgsl-rs` and you're done.

2. **Stable Rust.**

   No pinned nightly (but it also works on nightly). No waiting for upstream `rustc` changes.
   Your shaders compile with whatever stable toolchain you're already using.

3. **One compilation target.**

   Your shader code _is_ your Rust code. There's no separate SPIR-V compilation step, no binary artifacts
   to link back in. The WGSL is generated by a proc macro at compile time, as a string constant.

4. **The subset is explicit.**

   If you write Rust that can't be transpiled to WGSL, the `#[wgsl]` macro gives you a parse error.
   You don't discover limitations by hitting mysterious codegen failures deep in a compiler backend. 
   Instead, you discover them immediately, with a clear message, in your editor.

5. **Your shaders are testable.**

   Because the Rust code is fully operational on the CPU, you can unit test your shader logic with
   `cargo test`. And this is not just for testing! You can run the same code on the CPU in production if you
   want to. This is genuinely useful for things like physics, culling, or any logic you want to share
   between CPU and GPU.

6. **Generated `wgpu` linkage.**

   With the `linkage-wgpu` feature enabled, `wgsl-rs` generates buffer creation functions, bind group
   layouts, bind group constructors, shader module helpers, and vertex/fragment/compute state builders.
   All the boilerplate that you'd normally write by hand, derived directly from your shader declarations.

## What you give up

I mentioned the 80/20 rule, the [pareto principle](articles/adages.html).
Here's what you're trading away.

1. **WGSL only.**

   Rust-GPU targets SPIR-V, which runs on Vulkan, OpenGL, Metal (via MoltenVK), and more.
   `wgsl-rs` targets WGSL, which runs on WebGPU. In practice this covers all major platforms through
   `wgpu`, but you don't get access to Vulkan-specific extensions or features outside the WebGPU spec.

2. **A stricter subset of Rust.**

   No traits. **No generics**. No borrowing (except through the `ptr!` macro for function-scoped pointers). Very restricted
   module support -- you can only glob-import other `#[wgsl]` modules or `wgsl_rs::std`. No closures,
   no iterators, no `String`. It's a smaller sandbox than what Rust-GPU offers.

   The "no generics" part is honestly pretty tough.
   The `crabslab` changes I'm making on top of `wgsl-rs` have been tough because of this limitation. 

3. **Limited to WebGPU features.**

   No bindless resources (yet), no mesh shaders, and no ray tracing extensions.
   You only get what's in the WGSL spec.
   If you need cutting-edge GPU features, you'll need a different tool.

But here's the thing: `wgsl-rs` and Rust-GPU are not mutually exclusive. At least not over the evolution of
a software product.
You can start with `wgsl-rs`, get productive fast, and reach for Rust-GPU later if you hit a wall.
I'm actively working on making the two co-habitable.
Whether or not that becomes a reality I really can't say as just getting from zero-to-one is
a lot of work, but there is at least an easy migration path.

## What's next

This work is funded through [NLnet](https://nlnet.nl) under the
[NGI Zero Commons Fund](https://nlnet.nl/project/Renderling-Ecosystem/), which makes it possible for me
to work on this part-time. Thank you NLnet!

The roadmap for 2026:

* Take `wgsl-rs` to a **beta release on crates.io**.
* Adapt [`crabslab`](https://github.com/schell/crabslab) and `craballoc` to work with `wgsl-rs`, making
  GPU slab allocation easy and ergonomic.
* Build **`podecs`** -- a GPU-accelerated Entity Component System where components are plain old data defined
  with `wgsl-rs`, stored in GPU-accessible slabs, and systems run as compute shaders.
* **Rewrite Renderling's internals** on the new stack.
* Build a GPU ray tracer and work toward **real-time global illumination**.

If any of that sounds interesting, come check out the repos:

* [`wgsl-rs`]
* [`renderling`](https://github.com/schell/renderling)
* [`crabslab`](https://github.com/schell/crabslab)

Contributions, feedback, and questions are all welcome. Thanks for reading!

[`wgsl-rs`]: https://github.com/schell/wgsl-rs
