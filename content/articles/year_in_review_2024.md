---
title: Year in Review - 2024
date: 2025-01-09
---

# Year in Review - 2024

_What went wrong and what went right for Renderling in 2024.
Written 9 Jan, 2025_

<!-- 

My private stuff used for editing. 
Pay no attention to the man behind the curtain.

👍🤞🍖🚧🔗🤦🙇☕

...⏱️

😭😈😉😊

<video controls width="100%">
  <source src="" type="video/mp4" />
  Backup text.
</video>


<div class="images-horizontal">
    <div class="image">
        <label>Label</label>
        <img class="pixelated" width="100" src="" />
    </div>
</div>

<div class="image">
    <label>Label</label>
    <img
        width="750vw"
        src=""
        alt="" />
</div>
-->

o/ Hi y'all! 

Welcome to the Renderling wrap article for 2024. I'm hoping to make writing this article 
a tradition. 

This project started with its first commit in Sep 26, 2022 - so I've been 
working on this for roughly two years as a side project, after my day job and between 
raising two kids.

Without looking at the log of work, which is simply my list of closed PRs on GitHub, 
I can already tell you that I feel like I've accomplished a lot for Renderling this year!

## Sponsorships 💰

* [nlnet sponsorship](https://nlnet.nl/project/Renderling/)

  This has been amazing, and has really changed my perspective on open source. The 
  program has really been a guiding force for Renderling, and just knowing that people care 
  about the outcome enough to invest in the project makes the overall quality of the 
  software improve. Not to mention the fact that the money helps at a very basic level.
  I've already applied for 2025, 🤞. Thank you, nlnet!

  [<img src="https://nlnet.nl/logo/banner.png" alt="NLnet foundation logo" width="150" />](https://nlnet.nl) 
  [<img style="margin-left: 1em;" src="https://nlnet.nl/image/logos/NGI0_tag.svg" alt="NGI Zero Logo" width="150" />](https://nlnet.nl/core)

* [Second Half Games sponsorship](https://secondhalf.games/) 

  And more specifically Lucien from Second Half ;) 

  This sponsorship spun out of the work I was doing to unblock Renderling's shaders.

  You see, [`wgpu`](https://github.com/gfx-rs/wgpu) is a cross-platform graphics layer 
  that Renderling sits on top of. This layer abstracts over the popular graphics libraries like 
  DirectX, Vulkan, Metal, OpenGL, WebGL and WebGPU. It allows Renderling to target all platforms
  without too much platform-specific code.

  But it doesn't support all shader languages equally, and `wgpu`'s SPIR-V support was lacking 
  some features - notably support for atomics. 

  Now this is important for Renderling because Renderling's shaders are all written in Rust, 
  which then get compiled into SPIR-V, and atomics are an important building block when writing
  shaders. Oddly enough I've actually managed to avoid any use of atomics to this point, but 
  that's changing very soon.

  Anyway, long story short - Lucien saw that I was tackling atomic support in the SPIR-V frontend 
  of `wgpu`'s shader translator and sponsored me to help fast-track that work, and I'm happy to 
  say that [the work is done](https://github.com/gfx-rs/wgpu/issues/4489)!

  Thank you, Lucien! 

  And if you haven't checked out Second Half's game, 
  ["Meanwhile in Sector 80"](https://store.steampowered.com/app/2660180/MEANWHILE_IN_SECTOR_80/), 
  you should! It looks amazing!

  [<img src="https://renderling.xyz/img/second-half-logo.svg" alt="Second Half Games" width="150" />](https://secondhalf.games/)

* Other sponsorships 

  I also had a few other sponsorships, one on-going from my long-time collaborator and friend 
  Zach, and a generous one-time donation from John Nagle, who is working on 
  [Sharpview](https://www.animats.com/sharpview/), a metaverse viewer. 

  Also my buddy [James Harton](https://github.com/jimsynz) donated time on his machines for 
  dedicated CI hardware.

  Thank you, guys!

## Social 🤝   

* I've started fielding support questions on the Rust GameDev discord.

* There have been a couple reddit posts, not by me, mostly by John Nagle, aka Animats.

* GitHub stars have exploded this year:

<div class="image">
    <label>Stars as of the end of 2024</label>
    <img
        width="750vw"
        src="https://renderling.xyz/uploads/1736368354/star-history-202519.png"
        alt="Renderling GitHub stars, 2024" />
</div>

* I also became a maintainer of the [Rust-GPU project](https://github.com/rust-gpu).

Next year my social goals will be to get more and better documentation out there, with 
more examples. 

I'd also like to pull in some PRs from other folks, and possibly find a guest maintainer.
If 2025's nlnet grant goes through I'd like to contract some of the project's milestones 
out, as there's more work than I can manage myself, and having another person in the codebase
would be good for organization, and clarity and focus in the API.

I'd also like to write some small games 😈.

## Work 👷

Ok - let's enumerate the features and stuff added in 2024!

There were a lot of refactors and bug fixes, so I'm only going to mention the big rocks here.

* [nested transforms](https://github.com/schell/renderling/pull/95) 
  
  Support for scene hierarchy through nodes that contain other nodes, where a "node"
  is a rendering of some sort (a `Renderlet` in Renderling parlance)

* [physically based bloom](https://github.com/schell/renderling/pull/103)

  A new bloom implementation based on 
  [learnopengl's guest article](https://learnopengl.com/Guest-Articles/2022/Phys.-Based-Bloom)

* [rebuild of the animation system](https://github.com/schell/renderling/pull/108)

  Streamlined animation. Fixed some lingering bugs with rigging.

* [atlas uses texture array](https://github.com/schell/renderling/pull/121)  

  Support for multiple textures in the atlas. This greatly expanded the texturing capacity.

* [morph targets](https://github.com/schell/renderling/pull/126)

  Support for animations using morph targets. These are important for facial animations, among
  other things.

* [compute frustum culling](https://github.com/schell/renderling/pull/130)

  Pre-rendering step that removes out-of-view geometry. Good speedup.

* [WIP occlusion culling](https://github.com/schell/renderling/pull/137)

  This _would be_ a state of the art, two pass compute step to remove occluded geometry, but 
  I put it aside during the debugging phase. I'll come back to this in 2025.

* [cargo-gpu](https://github.com/Rust-GPU/cargo-gpu)

  I put this here even though it's not _exactly_ Renderling, but my shader compilation tools from 
  Renderling became the basis of this new, more general command line tool. Along with the work 
  of [Thomas Buckley-Houston](https://github.com/tombh). We essentially mashed our compilation 
  tools together to come up with `cargo-gpu`. Check it out!

## Website

Last but not least I have to mention that this entire bespoke website was set up this year, 
and I wrote 4300 lines in the devlog! 
That's a lot of debugging. 
But I do tend to make lots of newlines...

Here's the [renderling.xyz code](https://github.com/schell/renderling.xyz) if you care to 
take a look. 
It's very much made in a style that works for my workflow, and I'm often adding to it.

## Looking into 2025

On my docket for 2025 are more features, documentation and examples.

I'd also love to work with an artist to get some good renderings to show off.

I guess I could always use some off-the-shelf assets, though.

Here's my feature short-list:

* shadow mapping 
* finishing occlusion culling
* support for texture compression
* analytical light tiling 
* a pinch of raymarching
* screen-space ambient occlusion

## What went right and what went wrong

Just about everything I mentioned up to this point went right. I got a lot of features 
shipped in what I think is a pretty short amount of time. 

But one glaring difficulty I had was with occlusion culling. This wasn't part of my 
nlnet grant but I dedicated a good chunk of time to it. I felt like I was always just 
a couple hours away from finishing, but the rabbit hole got deeper and deeper. I think 
sinking this much time bumped a few other milestones out of reach for me in 2024 - namely
shadow mapping and light tiling. Next year I'll make it one of my milestones, if I win 
another grant. Then I can really focus on it without worry.

You can read about occlusion culling in the devlog 
[here](https://renderling.xyz/devlog/index.html#sat_nov_2_2024__sun_nov_3_2024).

Another problem is that packing in all these features has fractured the API a bit. The 
project doesn't feel very "well planned" to me... ...because it isn't! I've been working 
on just getting things to work and haven't put any polish on it. 
The APIs need a bit of work around the edges to ensure the pieces line up well. 

All in all, though, I'm very happy with this project :)

## And that's a wrap!

I'm going to keep this short and sweet, so that's it for 2024. 

Thanks for reading and following along. 

I wish you the best in 2025!

<3
