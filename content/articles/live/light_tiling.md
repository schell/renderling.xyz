---
title: Light Tiling, Live!
toc: true
---

_Following along with Renderling's initial implementation of light tiling_

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



# Introduction to light tiling - Sat 22 Mar 2025

I'm finally starting out on the feature that I created Renderling for - light tiling.
This will be the capstone on Renderling's "Forward+" approach to rendering.

The state of the art was introduced by Takahiro Harada, Jay McKee, and Jason Yang in their paper
["Forward+: Bringing Deferred Lighting to the Next Level"](https://takahiroharada.wordpress.com/wp-content/uploads/2015/04/forward_plus.pdf)...
...in 2012!
At the time of this writing, that's 13 years ago. Time flies like an arrow!
...and fruit flies like a banana.

When I read that paper I saw this screenshot, and was really impressed:

<div class="image">
    <label>A screenshot from the AMD Leo demo using Forward+</label>
    <img
        src="https://renderling.xyz/uploads/1742612068/Screenshot_2025-03-22_at_3.53.46PM.png"
        alt="A screenshot from the AMD Leo demo using Forward+" />
</div>

You can still see that demo [here, on YouTube](https://www.youtube.com/watch?v=C6TUVsmNUKI).

Anyway, on with the show.

## Method

Light tiling exists as a method to cut down the number of lights traversed during shading.
In a naive shading implementation every fragment must accumulate the light of every light
in the scene to determine the color of that fragment.
You can imagine that if your screen resolution is big, and you have a lot of lights, then
the number of lights visited in one frame of rendering can be huge.

Even for a small screen it's a giant number!

`_800pixels in width_ * _600pixels in height_ * _1024 lights_ = 491,520,000 light * pixels^2`

That's a fun unit, **light-pixels-squared**.
The unit itself really expresses the exponential nature of lighting calculations.

So, we have two options to cut down the number of light-pixels-squared:
1. Less pixels-squared.
2. Less light.

We can't make the picture smaller (we could, but that is not what we want).
So we'll cut down the number of lights.

But we want all our lights. Our lights are good lights. They deserve to illuminate.

We do know that most lights have a sphere or cone of illumination, though, and those lights don't
illuminate any fragments outside that illumination area.
I say "most" because directional lights do _not_ have a cone or sphere of illumination.
But at the same time we don't usually have that many directional lights, so we can ignore those.

So what we can do is this:
1. Divide the screen into small tiles
2. Calculate which lights illuminate the tile and store them in a list
3. Then in shading, traverse _that list_ for the fragments in the tile

Doing so will greatly reducing the number of lights we need to visit per fragment.

How all that gets done is different for each Forward+ renderer I've looked at, but they mostly share
these three steps:

1. **Depth pre-pass**
   Fill the depth buffer by rendering the scene geometry without a fragment shader.
2. **Light culling**
   Split the screen into tiles and compute the list of lights for each tile.
3. **Shading**
   Shade as normal, but for each fragment, use the light list of the corresponding tile instead of
   the global list of lights.

## Prior Art

* https://github.com/bcrusco/Forward-Plus-Renderer
* https://github.com/arnaudmathias/forward_plus_renderer
* https://wickedengine.net/2018/01/optimizing-tile-based-light-culling/comment-page-1/

Also, Godot and Unity both use Forward+.

# Let's do some Light Tiling - Sun 23 Mar 2025

I just realized that the **Depth pre-pass** step can be used for occlusion culling as well as
light culling, so this feature will pair well with the occlusion culling I have planned for
later this year. Maybe I should just include that here?

I'm a little worried about losing MSAA when rendering, but I guess the depth
pre-pass will render every object that passes frustum culling into the depth
map. Then we'll use some occlusion culling to only "redraw" the meshlets
that are in the scene and visible. We'll also compute the light tiles. Then
we'll do the redraw. During that redraw is when we'll do shading and MSAA will
resolve. So I _think_ that's all fine.

I could even implement light tiling _without the occlusion culling_ and it
would probably only bump the render time up a little. We'll see.

Anyway - let's do the depth pre-pass.

## Depth pre-pass work

I think what I'll do in order to get cracking as soon as possible (in pursuit
of [the 20/80 rule](/articles/adages.html#pareto_pricipal)), is just render
twice.
Once as the depth pre-pass and then once again for shading, with the
light culling in-between. 

...Huh. Now that I'm thinking of it, this also may be a good "spot" to put
order independent transparency.
* <https://learnopengl.com/Guest-Articles/2020/OIT/Introduction>
* <https://learnopengl.com/Guest-Articles/2020/OIT/Weighted-Blended>
* <https://casual-effects.blogspot.com/2014/03/weighted-blended-order-independent.html>
Eh - I'm not going to prematurely worry about that. But it's here when I need it.

Onward.

Here's the new depth pre-pass shader:
```rust
/// Depth pre-pass for the light tiling feature.
///
/// This shader writes all staged [`Renderlet`]'s depth into a buffer.
///
/// This shader is very much like [`shadow_mapping_vertex`], except that
/// shader gets its projection+view matrix from the light stored in a
/// `ShadowMapDescriptor`.
///
/// Here we want to render as normal forward pass would, with the `Renderlet`'s view
/// and the [`Camera`]'s projection.
///
/// ## Note
/// This shader will likely be expanded to include parts of occlusion culling and order
/// independent transparency.
#[spirv(vertex)]
pub fn light_tiling_depth_pre_pass(
    // Points at a `Renderlet`.
    #[spirv(instance_index)] renderlet_id: Id<Renderlet>,
    // Which vertex within the renderlet are we rendering?
    #[spirv(vertex_index)] vertex_index: u32,
    // The slab where the renderlet's geometry is staged
    #[spirv(storage_buffer, descriptor_set = 0, binding = 0)] geometry_slab: &[u32],
    // Output clip coords
    #[spirv(position)] out_clip_pos: &mut Vec4,
) {
    let renderlet = geometry_slab.read_unchecked(renderlet_id);
    if !renderlet.visible {
        // put it outside the clipping frustum
        *out_clip_pos = Vec3::splat(100.0).extend(1.0);
        return;
    }

    let camera_id = geometry_slab
        .read_unchecked(Id::<GeometryDescriptor>::new(0) + GeometryDescriptor::OFFSET_OF_CAMERA_ID);
    let camera = geometry_slab.read_unchecked(camera_id);

    let (_vertex, _transform, _model_matrix, world_pos) =
        renderlet.get_vertex_info(vertex_index, geometry_slab);

    *out_clip_pos = camera.view_projection() * world_pos.extend(1.0);
}
```
And at GitHub: <https://github.com/schell/renderling/pull/163/commits/a946f1aa911aa098b55d8c7ea1993cc8cb7da4df>

I imagine that later on I can add a fragment shader step that stores the
`Renderlet` `Id`s and does some more calculations for occlusion and transparency.

### Mon 24 Mar 2025

Here's the bit that computes the min and max depth of the tiles:

```rust
/// Compute the min and max depth of one fragment/invocation for light tiling.
pub fn light_tiling_compute_min_and_max_depth(
    frag_pos: UVec2,
    depth_texture: &impl Fetch<UVec2, Output = Vec4>,
    lighting_slab: &[u32],
    tiling_slab: &mut [u32],
) {
    // Depth frag value at the fragment position
    let frag_depth: f32 = depth_texture.fetch(frag_pos).x;
    // Fragment depth scaled to min/max of u32 values
    //
    // This is so we can compare with normal atomic ops instead of using the float extension
    let frag_depth_u32: u32 = (u32::MAX as f32 * frag_depth) as u32;
    // The tile's xy among all the tiles
    let tile_xy = UVec2::new(frag_pos.x / 16, frag_pos.y / 16);
    // The tile's index in all the tiles
    let tile_index = tile_xy.x * 16 + tile_xy.y;
    let tiling_desc = lighting_slab.read_unchecked(
        Id::<LightingDescriptor>::new(0) + LightingDescriptor::OFFSET_OF_LIGHT_TILING_DESCRIPTOR,
    );
    // index of the tile's min depth atomic value in the tiling slab
    let min_depth_index = tiling_desc.tile_depth_mins.at(tile_index as usize).index();
    // index of the tile's max depth atomic value in the tiling slab
    let max_depth_index = tiling_desc.tile_depth_maxs.at(tile_index as usize).index();

    let _prev_min_depth = unsafe {
        spirv_std::arch::atomic_u_min::<
            u32,
            { spirv_std::memory::Scope::Workgroup as u32 },
            { spirv_std::memory::Semantics::WORKGROUP_MEMORY.bits() },
        >(&mut tiling_slab[min_depth_index], frag_depth_u32)
    };
    let _prev_max_depth = unsafe {
        spirv_std::arch::atomic_u_max::<
            u32,
            { spirv_std::memory::Scope::Workgroup as u32 },
            { spirv_std::memory::Semantics::WORKGROUP_MEMORY.bits() },
        >(&mut tiling_slab[max_depth_index], frag_depth_u32)
    };
}
```

So far the shader entry point looks like this:

```rust
/// Light culling compute shader.
#[spirv(compute(threads(16, 16, 1)))]
pub fn light_tiling_compute_tiles(
    #[spirv(storage_buffer, descriptor_set = 0, binding = 0)] geometry_slab: &[u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 1)] lighting_slab: &[u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 2)] tiling_slab: &mut [u32],
    #[spirv(descriptor_set = 0, binding = 3)] depth_texture: &DepthImage2d,
    #[spirv(global_invocation_id)] global_id: UVec3,
) {
    let geometry_desc = geometry_slab.read_unchecked(Id::<GeometryDescriptor>::new(0));
    let size = geometry_desc.resolution;
    if !(global_id.x < size.x && global_id.y < size.y) {
        // if the invocation runs off the end of the image, bail
        return;
    }

    light_tiling_compute_min_and_max_depth(
        global_id.xy(),
        depth_texture,
        lighting_slab,
        tiling_slab,
    );
}
```

That commit is <https://github.com/schell/renderling/pull/163/commits/3cbf936b0ef2844bfd4e66c109f016dc7a5bee46>.

Next I'm going to run the depth pre-pass and confirm that the tiling slab
contains the correct values by exporting the min and max values as images.

For that I'm going to need a scene.

# Setting the scene - Thu 10 Apr 2025

I've created a simple scene of a moon-lit house on a plateau in the middle of the ocean.

<div class="image">
    <label>Moon-lit house, for testing light tiling</label>
    <img
        src="https://renderling.xyz/uploads/1744224134/off.png"
        alt="a simple scene of a moon-lit house on a plateau in the middle of the ocean" />
</div>

The moon is in front of the camera, so we get a little bit of reflection off the still water,
and the moon is casting a shadow on the front of the house.

This should help us see the lights that we'll be adding programmatically.

## Blender notes on adding custom properties - Tue 15 Apr 2025

In order to add lots of lights programmatically, and pseudo-randomly,
I'd like to place a few AABBs around the scene from which the lights
will spawn. To do this we need to add these AABBs as objects in the
GLTF file, generated from our Blender scene. Here's a quick discussion on
how to use the `extras` property on GLTF nodes
<https://github.com/omigroup/gltf-extensions/discussions/26>. 

## Resetting the scene - Sat 26 Apr, 2025

So I have a nice test case going, with a new simplified scene. Sorry for switching it up on you,
but the previous scene had some degenerate meshes. That's what I get for buying meshes instead of
making them myself...

So, here's the scene, in a couple forms, and as you can see, I'm programmatically adding 1024
point lights of varying size and colors.

<div class="image">
    <label>Scene without any lighting</label>
    <img
        src="https://renderling.xyz/uploads/1745638402/1-no-lighting.png"
        alt="a small scene with a couple shapes, including Suzanne, no lighting" />
</div>

<div class="image">
    <label>Scene with lighting and shadows, no extra point lights</label>
    <img
        src="https://renderling.xyz/uploads/1745638402/2-before-lights.png"
        alt="a small scene with a couple shapes, including Suzanne, with lighting" />
</div>

<div class="image">
    <label>Scene with lighting and shadows, plus point lights and meshes for the point lights</label>
    <img
        src="https://renderling.xyz/uploads/1745638402/3-after-lights.png"
        alt="a small scene with a couple shapes, including Suzanne, with lighting and extra point lights" />
</div>

<div class="image">
    <label>Scene with lighting and shadows, plus point lights, without meshes for the point lights</label>
    <img
        src="https://renderling.xyz/uploads/1745638402/4-after-lights-no-meshes.png"
        alt="a small scene with a couple shapes, including Suzanne, with lighting and extra point lights, but no meshes for the lights" />
</div>

And here is a graph of the frame timings of rendering this scene with a variable number of lights.
The time for rendering with only one light is pretty bad, we'll have to debug that later, but this gives
us a good baseline for our tiling.

<div class="image">
    <label>Scene frame timings</label>
    <img
        src="https://renderling.xyz/uploads/1745638402/frame-time.png"
        alt="frame timing graph of rendering the scene with different numbers of lights" />
</div>

You can see that performance is directly, linearly proportional to the number of lights in the scene.

...

About the poor performance with even only the default light - after looking at a GPU trace it looks like the main fragment shader is simply getting too large.
I'm doing too many arithmetic operations.
I think I can probably part some of this out, but not now.

...

So back to the tiling - I'll set up and integrate the shader I wrote before, which records the min
and max depth per tile.

This part takes a while. I wish I had some way of auto-generating this boilerplate.

...

Ok - I've hit a snag. Not so much a blocker but a bummer in that I need two copies of the compute shader
that computes the min and max depth of the light tiles.
This is because it reads from the depth texture, but that depth texture may or may not be multisampled.
The multisampled-ness of a depth texture is determined at compile time, which means I need two different
shaders.

Essentially this is the same problem I ran into with
[occlusion culling](https://renderling.xyz/devlog/index.html#about_those_hurdles)...

## Ensuring mins and maxes are reset - Sun 27 Apr, 2025

Today I made sure that the tile computing shader resets the mins and the maxes for each
tile before it compares depths.

It always surprises me how much verification I need before getting to the meat of a problem.

Today I got tripped up by the dimensions of the depth min and max arrays, and fixed it by
writing out the images of those arrays after clearing them with a shader. Turns out I was
calculating the size incorrectly as

```rust
depth_buffer_size / 16 + 1
```

...which added an extra column of tiles when the `depth_buffer_size` was evenly divisible by `16`.
The solution was to use (essentially):

```rust
(depth_buffer_size.into_f32s() / 16.0).ceil()
```

...that way when `depth_buffer_size` is evenly divisible by the tile size, no extra column is added,
which would mess up our later calculations.

Of course I had to **see** this in an image before I could tell what the bug was.

## Ensuring lights' frustums are calculated correctly - Sat 10 May, 2025

I'm down to the wire on my grant project!

I got a bit derailed this past week due to the fact that my partner and I are buying a house 🏡
here in New Zealand.

Today is the last day to finish this to hit the milestone...
...let's see how far I can get!

I left off at the part where we're running the shader that calculates frustums for each light tile and
gathers the lights that affect it. I have code that I _think_ should work, but the resulting images I'm
reading out of the calculated tiles are obviously incorrect, so I'm going to try to check the algo on the
CPU.

The difficulty here is that in SPIRV we can access the `tiling_slab: &mut [u32]` atomically without wrapping
it with anything (the atomic ops just take "pointers"), but on CPU we must pass an array of (something like)
`AtomicU32`. I'm not sure how to model this interaction to test it on the CPU.

I think I'll do the dumbest thing and use something like an `IsAtomicSlab` trait. At this point I don't care
about what's completely correct as I'm under a very tight time constraint.

...

I _think_ this is made easier by the fact that I don't really (at this point) have to test the atomic operation
of the algorithm, just the calculations inside it. So I don't have to run anything in parallel on the CPU.
Instead, I just have to be able to run the function on similar data to see where the error is - even if it's being
run synchronously.

...

Ok, turns out I'm not even that far yet. Let's back up and sanity check that I'm calculating the various indices
correctly, as this code is very index-heavy. 

So - for each fragment we calculate its position in the tile, then figure out which light it should check against
the frustum:

<div class="image">
    <label>Color shows lighting array index with 20 lights, represented by 20 colors</label>
    <img
        class="pixelated"
        width="1000vw"
        src="https://renderling.xyz/uploads/1746830599/step.png"
        alt="indices into the lighting array" />
</div>

But what about when there are more lights than there are indices in the tile?
We also have to check that a given fragment (invocation) visits more than one index in order to check all
lights:

<div class="image">
    <label>Shade shows number of lights visited by each invocation, lighter is more visits, tile size outlined, 1100 lights</label>
    <img
        class="pixelated"
        src="https://renderling.xyz/uploads/1746832361/Screenshot_2025-05-10_at_11.12.16AM.png"
        alt="indices into the lighting array" />
</div>

Ok, so I think the step width of the loop is correct.
We can see that the first fragments in the tile do one more lighting calculation than the rest.

Next I think we _will_ need atomic ops.

I'll go and set up the scene, read out all the slabs and then perform the shader on the CPU.

...

Ok, I have the depth image, the slabs, and I've mapped the atomic ops to synchronous ones on CPU.
Let's run the code on the CPU and then look at the resulting tiles...

...it looks like one of the light's node transforms has NaN values. Oof. Debugging time.

## End of my grant - Sun 11 May

Well, I've come to end of my nlnet grant period. 
I'm probably about half-finished on light tiling, if you include the time it would take to
work the proof of concept into the existing API.

I **will** keep working on it, and I'll see if I can get "partial credit" for the work thus far.

Hopefully Renderling is selected for another year of funding and I can finish up light tiling before
that starts. If not, I'll finish it up regardless :)

Stay tuned.

## Continuing on - Sun 18 May

While investigating why some light node's transforms have `NaN` values, I discovered that the transforms are
somehow not being applied correctly.

...

Ok, first thing here, [I found and fixed an issue where the main renderlet bind group was not being
properly invalidated](https://github.com/schell/renderling/pull/166/commits/8730f76dd679dd7c6105b181b2cb2151a4b4ac79#diff-7eb217a8e32eb647c8f5758a3b1f555aade82e629b4f35b4b6f5fe0679709a11L203).

This is one of the 2 hardest problems in computing:
1. cache invalidation
2. naming things
3. off-by-one errors

Now that that is fixed, I can continue debugging the transform NaNs.

## Point and spotlight discrepancies - Fri 11 July 2025

I wrote a new test that shows some geometry lit by three lights. Directional, point and spot, respectively.

Each light is placed at `(1, 1, 1)`, about `1` above the upper right corner of the geometry.

In the directional and spotlight cases, they each point at the origin.

<div class="images-horizontal">
    <div class="image">
        <label>Directional</label>
        <img class="pixelated" src="https://renderling.xyz/uploads/1752185721/directional.png" />
    </div>
    <div class="image">
        <label>Point</label>
        <img class="pixelated" src="https://renderling.xyz/uploads/1752185721/point.png" />
    </div>
    <div class="image">
        <label>Spot</label>
        <img class="pixelated" src="https://renderling.xyz/uploads/1752185721/spot.png" />
    </div>
</div>

You can see above that the directional light looks fine.
The point light, however, seems to be reflecting light incorrectly, as if the light direction were
flipped. It's also not reflecting light on the horizontal walls of the scenery.
Likewise, the spotlight's reflections don't show anything on the vertical walls of the scenery.

My guess is that the direction vectors are not being calculated correctly.
The place to look is in the `renderling::pbr::shade_fragment` function.

In that function we loop over all the lights and accumulate the radiance.
Inside that loop I can see this code for calculating the radiance of a point light:

```rust
        // determine the light ray and the radiance
        let (radiance, shadow) = match light.light_type {
            LightStyle::Point => {
                let PointLightDescriptor {
                    position,
                    color,
                    intensity,
                } = light_slab.read(light.into_point_id());
                // `position` is the light positios
                let position = transform.transform_point3(position);
                // `in_pos` is the fragment's surface position
                let frag_to_light = in_pos - position; // - in_pos;
```

From the looks of that last line there, it seems there's been some confusion about the `frag_to_light` direction.

If I flip that calculation to be `position - in_pos`, we get the point light rendering to look more like the
spotlight rendering:


<div class="images-horizontal">
    <div class="image">
        <label>Point</label>
        <img class="pixelated" src="https://renderling.xyz/uploads/1752189853/point.png" />
    </div>
    <div class="image">
        <label>Spot</label>
        <img class="pixelated" src="https://renderling.xyz/uploads/1752189853/spot.png" />
    </div>
</div>

I bet `frag_to_light` got flipped while I was thrashing on something. After we figure out the vertical
reflection issue I'll run all the tests again and see what has broken.

Now that I think about it, it's actually correct that no light should be shown on the vertical walls.
Since the light is directly over the corner of the model, the rays would be collinear with the wall surface at best.
I can move the light a little further out, like to `(1.1, 1.0, 1.1)` and then we should see a little
light being shown on the walls.

<div class="images-horizontal">
    <div class="image">
        <label>Point</label>
        <img class="pixelated" src="https://renderling.xyz/uploads/1752190946/point.png" />
    </div>
    <div class="image">
        <label>Spot</label>
        <img class="pixelated" src="https://renderling.xyz/uploads/1752190946/spot.png" />
    </div>
</div>

Phwew! Glad I thought of that. I could have spun my wheels on that for a while.

Now we can get back to tiling.

## Chasing NaN - Sat 12 July 2025

Now I'm back chasing down the transform NaN bug.

It's pretty obvious what's happening here:

1. I have 3 slabs (which are like arenas) where I allocate things on the GPU (and sync some of them on the CPU).
2. Most transforms live on the "geometry" slab, which contains things like mesh vertex data.
3. Light transforms live on the "lighting" slab, including their transforms.
4. Light tiling was reading light transforms from the "geometry" slab.

Boom. That's that.

But how am I going to solve this problem?
I could just move the transform read from the "geometry" slab to the "lighting" slab and be done with it, but
I _will_ run into more problems like this.
It seems apparent to me that a slab should carry some compile-time data that it imparts to values that live on
it.
This way we know at compile time which values live on which slabs, and this error would not happen.
Something like this:

```rust
struct SlabAllocator<T: SingleSlabOrigin = DefaultSingleSlabOrigin> {...}

struct Hybrid<T: SlabItem, Origin: SingleSlabOrigin = DefaultSingleSlabOrigin> {...}
```

This might also make it possible to have values which live on more than one slab, by doing something like:

```rust
struct MultiOriginHybrid<T: SlabItem, O: MultiSlabOrigin = DefaultMultiSlabOrigin>{...}

let geometry_slab: SlabAllocator<GeometryOrigin> = SlabAllocator::new(...);
let lighting_slab: SlabAllocator<LightingOrigin> = SlabAllocator::new(...);
let hybrid: Hybrid<u32, GeometryOrigin> = geometry_slab.new_value(666);
let multi: MultiOriginHybrid<u32, (GeometryOrigin, LightingOrigin)> = hybrid.into_multi(&lighting_slab);
```

Well, I've made a note of it. I'm not going to pursue that yet. For now I'm just switching the read.

## Back to tiling - Sun 13 July 2025

Yesterday I found out that I was confused about where the light transforms would live.
This is one problem with having many different slabs with similar data, it's hard to see which slab
a value belongs to.

Because of this I think that the API for creating an analytical light must change. Specifically I think
it should **not** take a `NestedTransform` - because those are allocated on the geometry slab.

I don't think it's reasonable to expect the library user to remember which slab to create a value on.
It should be handled internally, or with types.

For this reason I'll make it so `AnalyticalLightBundle` creates its own `NestedTransform` on the light
slab which can be updated after creation.

## Sun 20 July 2025

My schedule is still pretty hectic after changing jobs and moving house.
I haven't settled into a rhythm yet.

I've added some helpers to `Camera` and the `renderling::math` module.

## Fri 25 July 2025

I've added a number of improvements to `Camera` and `Frustum` to make the tests for light tiling easier
to look at, mostly for linearizing and normalizing depth.

The amount of sanity checking the camera transformations require really make me wish I had gone with an enum for
`Camera` that could determine if the projection was orthographic or perspective.

I might try that in the future.
The main reason it's not like that is because it would require more slab reads in shaders.

### Ensuring tiles are cleared

The first stage of light tiling is clearing the tiles of the previous frame's data.
I've written a test to ensure that this stage happens properly.

The test involves first writing known data to the tiling slab.
Here you can see a visualization of that data:

<div class="images-horizontal">
    <div class="image">
        <label>Number of lights illuminating the tile, as noise</label>
        <img class="pixelated" width="100%" src="https://renderling.xyz/uploads/1753403473/1-lights.png" />
    </div>
    <div class="image">
        <label>Depth minimums, as x,y distance from the bottom right</label>
        <img class="pixelated" width="100%" src="https://renderling.xyz/uploads/1753403473/1-mins.png" />
    </div>
    <div class="image">
        <label>Depth maximums, as x,y distance from upper left</label>
        <img class="pixelated" width="100%" src="https://renderling.xyz/uploads/1753403473/1-maxs.png" />
    </div>
</div>

Now we should be able to run the `clear_tiles` shader and see that the minimums are set to a max value,
the maximums are set to a minimum value (`0`) and the number of lights set to `0`.

Blaarg! It seems that calling the `clear_tiles` shader is zeroing-out the `LightTilingDescriptor`, which makes
our tiles unreachable. This is likely due to faulty pointer math.

Indeed, it turns out the slab was getting clobbered, so I simplified the case by breaking it out into its own
shader, and now we get some proper clearing:

<div class="images-horizontal">
    <div class="image">
        <label>Number of lights illuminating the tile, cleared to `0`</label>
        <img class="pixelated" width="100%" src="https://renderling.xyz/uploads/1753405507/2-lights.png" />
    </div>
    <div class="image">
        <label>Depth minimums, cleared to max</label>
        <img class="pixelated" width="100%" src="https://renderling.xyz/uploads/1753405507/2-mins.png" />
    </div>
    <div class="image">
        <label>Depth maximums, cleared to min</label>
        <img class="pixelated" width="100%" src="https://renderling.xyz/uploads/1753405507/2-maxs.png" />
    </div>
</div>

The next step is ensuring the min and max depth are being calculated properly.

### Ensuring min and max depth calculations

For this I'm going to break out the min and max depth computation into its own shader.
I did this for clearing tiles earlier, and it makes things much easier to verify.

After that it looks like everything is copacetic, we can see the minimums and the maximums
clearly, and the minimums look closer, as they should (depth 1.0 is on the far plane):

<div class="images-horizontal">
    <div class="image">
        <label>The scene</label>
        <img class="pixelated" width="100%" src="https://renderling.xyz/uploads/1753420111/1-scene.png" />
    </div>
    <div class="image">
        <label>Depth minimums, the furthest parts of the visible scene</label>
        <img class="pixelated" width="100%" src="https://renderling.xyz/uploads/1753420111/2-mins.png" />
    </div>
    <div class="image">
        <label>Depth maximums, the closest parts of the visible scene</label>
        <img class="pixelated" width="100%" src="https://renderling.xyz/uploads/1753420111/2-maxs.png" />
    </div>
</div>

Now we're ready to calculate the light lists.

## Sat 26 July, 2025

### Ensuring the light bins 

The last shader in the tiling workflow is the one that calculates which lights affect each tile.

It's easily the trickiest shader because it does two things:

1. Calculate the AABB frustum in NDC coordinates of the tile according to the min and max depths computed
   earlier.
2. Iterate over each light...
    - Calculate the AABB frustum of the light's illumination 
    - Add the light to the bin if the two frustums intersect

Both steps include math that is easy to get wrong and then step 2 has atomic ops which can be spooky.
It also iterates over a subset of lights for each invocation in the tile, and that iteration could
be broken.

But at least I'm getting _some kind of result_.
Here is that result, where the second image shows the number of lights in each tile's bin:


<div class="images-horizontal">
    <div class="image">
        <label>The scene</label>
        <img class="pixelated" width="100%" src="https://renderling.xyz/uploads/1753479030/1-scene.png" />
    </div>
    <div class="image">
        <label>Length of each tile's light bin, normalized</label>
        <img class="pixelated" width="100%" src="https://renderling.xyz/uploads/1753479030/2-lights.png" />
    </div>
</div>

I really should get one of those nifty widgets that has a slider to show the two images overlapping each other.

<!--
TODO: make one of those slider thingys
-->

As you can see, there _are different values_.

That's good because we know the shader is doing _something_.

But they're not the values we expect.

There's only one directional light in this scene, so we should expect that every tile has one light in it.
That would mean that every pixel in the second image should be white.

I'm going to get some more debug info.

**Oof**. I've got a bug to chase.

It looks like each time the test runs I get a different set of bins:

<div class="images-horizontal">
    <div class="image">
        <label>Run 1, length of each tile's light bin, normalized</label>
        <img class="pixelated" width="100%" src="https://renderling.xyz/uploads/1753480081/2-lights-0.png" />
    </div>
    <div class="image">
        <label>Run 2, length of each tile's light bin, normalized</label>
        <img class="pixelated" width="100%" src="https://renderling.xyz/uploads/1753480081/2-lights-1.png" />
    </div>
    <div class="image">
        <label>Run 3, length of each tile's light bin, normalized</label>
        <img class="pixelated" width="100%" src="https://renderling.xyz/uploads/1753480081/2-lights-2.png" />
    </div>
</div>

So what could it be? What is introducing this non-determinism?

I wouldn't be surprised if the algorithm has a bug, but since we're not in control of the order of invocations
on the GPU, the bug results in a different "bin pattern" each time it's run. The atomic operation makes the order
important, for lack of a better word, so I could see any error in one invocation compounding over the entire run.

My first hunch is that iteration is broken in some way that lets multiple invocations within the same tile consider
the same light, meaning one light might get added to the bin more than once. I mean, something like that _must_ be
happening because we only have one light, and there are multiples stored in the image with gray in it.

So the first thing to do is sanity check our iteration.

I've created a helper to iterate over the lights:

```rust
/// Helper for determining the next light to check during an
/// invocation of the light list computation.
struct NextLightIndex {
    current_step: usize,
    stride: usize,
    lights: Array<Id<Light>>,
    global_id: UVec3,
}

impl Iterator for NextLightIndex {
    type Item = Id<Id<Light>>;

    fn next(&mut self) -> Option<Self::Item> {
        let next_index = self.next_index();
        self.current_step += 1;
        if next_index < self.lights.len() {
            Some(self.lights.at(next_index))
        } else {
            None
        }
    }
}

impl NextLightIndex {
    pub fn new(global_id: UVec3, analytical_lights_array: Array<Id<Light>>) -> Self {
        let stride =
            (LightTilingDescriptor::TILE_SIZE.x * LightTilingDescriptor::TILE_SIZE.y) as f32;
        Self {
            current_step: 0,
            stride: stride as usize,
            lights: analytical_lights_array,
            global_id,
        }
    }

    pub fn next_index(&self) -> usize {
        // Determine the xy coord of this invocation within the _tile_
        let frag_tile_xy = self.global_id.xy() % LightTilingDescriptor::TILE_SIZE;
        // Determine the index of this invocation within the _tile_
        let offset = frag_tile_xy.y * LightTilingDescriptor::TILE_SIZE.x + frag_tile_xy.x;
        self.current_step * self.stride + offset as usize
    }
}
```

I've also written a sanity check:

```rust
    #[test]
    fn next_light_sanity() {
        {
            let lights_array = Array::new(0, 1);
            // When there's only one light we only need one invocation to check that one light
            // (per tile)
            let mut next_light = NextLightIndex::new(UVec3::new(0, 0, 0), lights_array);
            assert_eq!(Some(0u32.into()), next_light.next());
            assert_eq!(None, next_light.next());
            // The next invocation won't check anything
            let mut next_light = NextLightIndex::new(UVec3::new(1, 0, 0), lights_array);
            assert_eq!(None, next_light.next());
            // Neither will the next row
            let mut next_light = NextLightIndex::new(UVec3::new(0, 1, 0), lights_array);
            assert_eq!(None, next_light.next());
        }
        {
            let lights_array = Array::new(0, 2);
            // When there's two lights we need two invocations
            let mut next_light = NextLightIndex::new(UVec3::new(0, 0, 0), lights_array);
            assert_eq!(Some(0u32.into()), next_light.next());
            assert_eq!(None, next_light.next());
            // The next invocation checks the second light
            let mut next_light = NextLightIndex::new(UVec3::new(1, 0, 0), lights_array);
            assert_eq!(Some(1u32.into()), next_light.next());
            assert_eq!(None, next_light.next());
            // The next one doesn't check anything
            let mut next_light = NextLightIndex::new(UVec3::new(2, 0, 0), lights_array);
            assert_eq!(None, next_light.next());
        }
        {
            // With 256 lights (16*16), each fragment in the tile checks exactly one light
            let lights_array = Array::new(0, 16 * 16);
            for y in 0..16 {
                for x in 0..16 {
                    let mut next_light = NextLightIndex::new(UVec3::new(x, y, 0), lights_array);
                    let next_index = next_light.next_index();
                    assert_eq!(Some(next_index.into()), next_light.next());
                    assert_eq!(None, next_light.next());
                }
            }
        }
    }
```

And after playing around with the tests I'm pretty confident the problem isn't the iteration.

So my next hunch is that it's something to do with the atomic ops...

...Okay, so I think I found it (I found _something_).

The code I had written for the last part of binning was this:

```rust
if should_add {
    // If the light should be added to the bin, get the next available index in the bin,
    // then write the id of the light into that index.
    let next_index = tiling_slab.atomic_i_increment::<
        { spirv_std::memory::Scope::Workgroup as u32 },
        { spirv_std::memory::Semantics::WORKGROUP_MEMORY.bits() },
    >(next_light_id);
    if next_index as usize >= tile_lights_array.len() {
        // We've already filled the bin, abort
        break;
    }
    // Get the id that corresponds to the next available index in the bin
    let binned_light_id = tile_lights_array.at(next_index as usize);
    // Write to that location
    tiling_slab.write(next_index.into(), &binned_light_id);
}
```

Unfortunately this is where my slab implementation tripped me up.
The last line is wrong:

```rust
    // Write to that location
    tiling_slab.write(next_index.into(), &binned_light_id);
```

Here, `next_index` is being turned into an `Id` and then we're writing the `binned_light_id`
into it.
But that's wrong - `binned_light_id` is what we should be writing to, and `light_id` is the thing we want to write.
So we were clobbering some data.

Now with that fixed it looks like we're writing the correct data.

### Zero volume frustum optimization 

One thing I can tell right off the bat is that there are sections of the scene where there is no scenery.
In those places the minimum depth matches the max depth, which essentially creates a frustum of zero volume.
A frustum with zero volume can't be illuminated, and so we can safely skip the entire tile in this case.

Here's an example of one of these tiles:

```
LightTile {
    depth_min: 1.0,
    depth_max: 1.0,
    next_light_index: 1,
    lights_array: Array<crabslab::id::Id<renderling::light::Light>>(8164, 32),
}
```

The `next_light_index` should be `0` here, because **no lights can illuminate the space**.

Also, this is consistent within the entire tile, so the binning shader should get a consistent bump
in performance for early exiting in the case of a zero volume frustum.

After that optimization we get a light-binning visualization like this:

<div class="images-horizontal">
    <div class="image">
        <label>The scene</label>
        <img class="pixelated" width="100%" src="https://renderling.xyz/uploads/1753479030/1-scene.png" />
    </div>
    <div class="image">
        <label>Number of lights illuminating a tile, normalized</label>
        <img
            src="https://renderling.xyz/uploads/1753494858/2-lights.png"
            width="100%"
            class="pixelated" />
    </div>
</div>

That looks pretty good!
It's what we would expect - there's one light binned on each tile where there is scenery.

## Sun 27 July, 2025

### Running it on our scene

So now it's time to hook it up to our fancy thousands-of-lights scene.

I'm going to rework it a bit to store all the tiling stuff on the lighting slab, as that is fairly small even
with thousands of lights.
During the build-out I've been maintaining a separate slab for tiling.
I'm not sure why I did that to begin with.

...

Okay, done. But I can already see something funky.

<div class="images-horizontal">
    <div class="image">
        <label>Depth minimums, normalized</label>
        <img class="pixelated" width="280vw" src="https://renderling.xyz/uploads/1753558125/5-mins.png" />
    </div>
    <div class="image">
        <label>Depth maximums, normalized</label>
        <img class="pixelated" width="280vw" src="https://renderling.xyz/uploads/1753558125/5-maxs.png" />
    </div>
    <div class="image">
        <label>Number of lights illuminating a tile, normalized</label>
        <img class="pixelated" width="280vw" src="https://renderling.xyz/uploads/1753558125/5-lights.png" />
    </div>
</div>

The minimums look like data from the number of lights, or something, and the number of lights are inverted from that.

Definitely wrong.
I'll try breaking out each tiling step (clearing, finding minimum/maximum, and binning) to see what's going on.
