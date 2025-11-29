---
title: devlog
---
_The latest happenings in renderling-land_

<!-- 

My private stuff used for editing. 
Pay no attention to the man behind the curtain.

👍🤞🍖🚧🔗🤦🙇☕

...⏱️

😭😈😉😊

🇳🇿 🏝️

-->

## Sun 30 Nov, 2025

The past two months I've been pretty slammed with my new job and family stuff,
but I've still been working on this project, albeit a bit sporadically.

### `crabslab` changes
I've managed to get a large PR finished on [`crabslab`](https://github.com/schell/crabslab/pull/5)
that provides synchronization of values changed by shaders on the GPU back to
their CPU caches.

This change has been a long time coming, and it unlocks a lot of potential for
interesting changes in Renderling, and beyond.

Specifically these changes help in any situation where a shader computes some
data that then gets used on the CPU.
And it does that with a minimal number of writes to and from the GPU.

This is all part of a grand scheme to blur the lines between GPU and CPU, and
make GPU programming easier, which is essentially the main goal behind
next year's worth of Renderling work.

### 2026 NLNet project

Renderling was selected for another year of funding by
[NLnet](https://nlnet.nl/project/Renderling-Ecosystem/).
I'm currently working on the project plan and we have double the funding of
last year, which means I'll be able to take on paid contributors and
get more accomplished in the same amount of time.

Next year's work is focusing on the ecosystem and then riding that rising tide
towards global illumination.

Global illumination is a very lofty goal, especially for a renderer that targets
the web, but I think we can get there with some key tradeoffs.

## Wed 24 September, 2025

I've pushed the [lighting chapter of the manual](/manual/lighting.html) live.

Part of this was fixing a bug in image based lighting regarding bindgroup invalidation. 

## Sun 21 September, 2025

I've done a rework of the API, greatly improved the documentation and created a user's manual
full of examples.
See the [PR for the API change and initial manual w/ examples here](https://github.com/schell/renderling/pull/199).

I've also made somewhat drastic website changes:

* The [manual](/manual/index.html) is now hosted here!
* The [docs](/docs/renderling/index.html) are now hosted here!
* The devlog is deprecated in favor of this news page.

### The devlog is no more

I'm moving away from the super long devlog format to something that hopefully is a bit
more focused.

Now the devlog is broken up into two "things":

1. Small news blurbs on this "news" page.
2. Long-form stream of consciousness "devlog" articles for specific features that might
   span multiple days, weeks, or months.

#### why

The devlog was getting huge, and it's a bit too chaotic.
Now I'll be live-blogging feature development in specific articles, like I did for
[Light Tiling, Live](/articles/live/light_tiling.html).
I'm hoping this does a better job of keeping the devlogs on-topic instead of being a big jumble
of different things.

All other blurbs are news, so they can live on the news page.
I'm just trying to stay organized as this project grows.

But - the old devlog will stay where it was, at [devlog](/devlog/index.html), for posterity and so the links still work.

### User's manual

[Check out the manual here](/manual/index.html).

The manual covers the basics, but lacks lighting examples.

Obviously it's a work in progress.

I'll be making sure that it's complete after getting it online.
Even in its incomplete state, it has lots of workable, tested examples in it,
and I hope it helps folks get up and running with the library.

### Documentation updates

There's a **ton** more documentation coverage. I did a big audit of the current documentation
and added more where needed as well as revamped the existing docs.

The latest docs are now hosted [here at /docs](./docs), due to the fact that Renderling
depends on a not yet released version of `spirv-std`, which the Rust-GPU group is still 
working on releasing.

### API changes

**I've removed the `crabslab::Id` and `craballoc::Hybrid*` types from the public API.**

I figured that it shouldn't be necessary for users to understand anything about slabs and
descriptors.

**Builder patterns**

Now all the various resources (`Primitive`, `Material`, `Vertices` etc) adhere to a builder
pattern for configuration and updates.

**`Skybox` is now separate from `Ibl`.**

Up until this point, if you wanted to render a skybox, that skybox would also perform
image based lighting automatically.
I decided to decouple these now, as there are valid situations where you may not want
IBL, but do want a skybox.

