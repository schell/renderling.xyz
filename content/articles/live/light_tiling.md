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
        <img class="pixelated" width="280vw" src="https://renderling.xyz/uploads/1753403473/1-lights.png" />
    </div>
    <div class="image">
        <label>Depth minimums, as x,y distance from the bottom right</label>
        <img class="pixelated" width="280vw" src="https://renderling.xyz/uploads/1753403473/1-mins.png" />
    </div>
    <div class="image">
        <label>Depth maximums, as x,y distance from upper left</label>
        <img class="pixelated" width="280vw" src="https://renderling.xyz/uploads/1753403473/1-maxs.png" />
    </div>
</div>

Now we should be able to run the `clear_tiles` shader and see that the minimums are set to a max value,
the maximums are set to a minimum value (`0`) and the number of lights set to `0`.

Blaarg! It seems that calling the `clear_tiles` shader is zeroing-out the `LightTilingDescriptor`, which makes
our tiles unreachable. This is likely due to faulty pointer math.

Indeed, it turns out the slab was getting clobbered, so I simplified the case by breaking it out into its own
shader, and now we get some proper clearing:

### A working clear_tiles

<div class="images-horizontal">
    <div class="image">
        <label>Depth minimums, cleared to max</label>
        <img class="pixelated" width="280vw" src="https://renderling.xyz/uploads/1753405507/2-mins.png" />
    </div>
    <div class="image">
        <label>Depth maximums, cleared to min</label>
        <img class="pixelated" width="280vw" src="https://renderling.xyz/uploads/1753405507/2-maxs.png" />
    </div>
    <div class="image">
        <label>Number of lights illuminating the tile, cleared to `0`</label>
        <img class="pixelated" width="280vw" src="https://renderling.xyz/uploads/1753405507/2-lights.png" />
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
        <img class="pixelated" width="280vw" src="https://renderling.xyz/uploads/1753420111/1-scene.png" />
    </div>
    <div class="image">
        <label>Depth minimums, the furthest parts of the visible scene</label>
        <img class="pixelated" width="280vw" src="https://renderling.xyz/uploads/1753420111/2-mins.png" />
    </div>
    <div class="image">
        <label>Depth maximums, the closest parts of the visible scene</label>
        <img class="pixelated" width="280vw" src="https://renderling.xyz/uploads/1753420111/2-maxs.png" />
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
        <img class="pixelated" width="480vw" src="https://renderling.xyz/uploads/1753479030/1-scene.png" />
    </div>
    <div class="image">
        <label>Length of each tile's light bin, normalized</label>
        <img class="pixelated" width="480vw" src="https://renderling.xyz/uploads/1753479030/2-lights.png" />
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
        <img class="pixelated" width="280vw" src="https://renderling.xyz/uploads/1753480081/2-lights-0.png" />
    </div>
    <div class="image">
        <label>Run 2, length of each tile's light bin, normalized</label>
        <img class="pixelated" width="280vw" src="https://renderling.xyz/uploads/1753480081/2-lights-1.png" />
    </div>
    <div class="image">
        <label>Run 3, length of each tile's light bin, normalized</label>
        <img class="pixelated" width="280vw" src="https://renderling.xyz/uploads/1753480081/2-lights-2.png" />
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
        <img class="pixelated" width="450vw" src="https://renderling.xyz/uploads/1753479030/1-scene.png" />
    </div>
    <div class="image">
        <label>Number of lights illuminating a tile, normalized</label>
        <img
            src="https://renderling.xyz/uploads/1753494858/2-lights.png"
            width="450vw"
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
        <label>Depth minimums</label>
        <img class="pixelated" width="280vw" src="https://renderling.xyz/uploads/1753558125/5-mins.png" />
    </div>
    <div class="image">
        <label>Depth maximums</label>
        <img class="pixelated" width="280vw" src="https://renderling.xyz/uploads/1753558125/5-maxs.png" />
    </div>
    <div class="image">
        <label>Number of lights</label>
        <img class="pixelated" width="280vw" src="https://renderling.xyz/uploads/1753558125/5-lights.png" />
    </div>
</div>

The minimums look like data from the number of lights, or something, and the number of lights are inverted from that.

Definitely wrong.
I'll try breaking out each tiling step (clearing, finding minimum/maximum, and binning) to see what's going on.

Okay, here are the results of each step.

#### Clear tiles

<div class="images-horizontal">
    <div class="image">
        <label>Depth minimums</label>
        <img class="pixelated" width="280vw" src="https://renderling.xyz/uploads/1753559766/5-mins.png" />
    </div>
    <div class="image">
        <label>Depth maximums</label>
        <img class="pixelated" width="280vw" src="https://renderling.xyz/uploads/1753559766/5-maxs.png" />
    </div>
    <div class="image">
        <label>Number of lights</label>
        <img class="pixelated" width="280vw" src="https://renderling.xyz/uploads/1753559766/5-lights.png" />
    </div>
</div>

#### Compute depth

<div class="images-horizontal">
    <div class="image">
        <label>Depth minimums</label>
        <img class="pixelated" width="280vw" src="https://renderling.xyz/uploads/1753559787/5-mins.png" />
    </div>
    <div class="image">
        <label>Depth maximums</label>
        <img class="pixelated" width="280vw" src="https://renderling.xyz/uploads/1753559787/5-maxs.png" />
    </div>
    <div class="image">
        <label>Number of lights</label>
        <img class="pixelated" width="280vw" src="https://renderling.xyz/uploads/1753559787/5-lights.png" />
    </div>
</div>

#### Compute bins

<div class="images-horizontal">
    <div class="image">
        <label>Depth minimums</label>
        <img class="pixelated" width="280vw" src="https://renderling.xyz/uploads/1753559795/5-mins.png" />
    </div>
    <div class="image">
        <label>Depth maximums</label>
        <img class="pixelated" width="280vw" src="https://renderling.xyz/uploads/1753559795/5-maxs.png" />
    </div>
    <div class="image">
        <label>Number of lights</label>
        <img class="pixelated" width="280vw" src="https://renderling.xyz/uploads/1753559795/5-lights.png" />
    </div>
</div>

Clearly the first step isn't going well.
[Remember what it should look like](#a-working-cleartiles).

By reading out the visualizations before running `clear_tiles` I can see that it's all zeroed-out.
So I don't think that's the problem.

If it were a problem in the shader we would expect our previous tests to fail...

...unless the shader is taking a different path...

...or we're doing something wrong when _calling_ the shader.

Let's pack in some known data for the tiles like we did to test `clear_tiles` and generate our visualizations.

Bingo!

#### After clearing pre-packed tiles

<div class="images-horizontal">
    <div class="image">
        <label>Depth minimums</label>
        <img class="pixelated" width="280vw" src="https://renderling.xyz/uploads/1753561188/5-mins.png" />
    </div>
    <div class="image">
        <label>Depth maximums</label>
        <img class="pixelated" width="280vw" src="https://renderling.xyz/uploads/1753561188/5-maxs.png" />
    </div>
    <div class="image">
        <label>Number of lights</label>
        <img class="pixelated" width="280vw" src="https://renderling.xyz/uploads/1753561188/5-lights.png" />
    </div>
</div>

You can clearly see that the shader is only clearing a 32x32 pixel space.
The problem is likely the shader invocation.

The invocation itself is fairly simple:

```rust
    pub(crate) fn clear_tiles(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        bindgroup: &wgpu::BindGroup,
    ) {
        let mut compute_pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("light-tiling-clear-tiles"),
            timestamp_writes: None,
        });
        compute_pass.set_pipeline(&self.clear_tiles_pipeline);
        compute_pass.set_bind_group(0, bindgroup, &[]);

        let x = (LightTilingDescriptor::TILE_SIZE.x / 16) + 1;
        let y = (LightTilingDescriptor::TILE_SIZE.y / 16) + 1;
        let z = 1;
        compute_pass.dispatch_workgroups(x, y, z);
    }
```

The calculations for the number workgroups is off.

Instead of:

```rust
let x = (LightTilingDescriptor::TILE_SIZE.x / 16) + 1;
```

It should be that `x` is width of the tile grid.

```rust
// Something like...
let x = tile_dimensions.x;
```

We need _exactly one invocation per tile_, and the size of the grid of tiles depends on the
size of the depth texture as well as the size of each tile.

It was just a coincidence that this code worked before.

The fixed invocation looks like this:

```rust
    pub(crate) fn clear_tiles(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        bindgroup: &wgpu::BindGroup,
        depth_texture_size: UVec2,
    ) {
        let mut compute_pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("light-tiling-clear-tiles"),
            timestamp_writes: None,
        });
        compute_pass.set_pipeline(&self.clear_tiles_pipeline);
        compute_pass.set_bind_group(0, bindgroup, &[]);

        let dims_f32 = depth_texture_size.as_vec2() / LightTilingDescriptor::TILE_SIZE.as_vec2();
        let workgroups = (dims_f32 / 16.0).ceil().as_uvec2();
        let x = workgroups.x;
        let y = workgroups.y;
        let z = 1;
        compute_pass.dispatch_workgroups(x, y, z);
    }
```

The `16.0` constant there comes from the fact that each workgroup has dimensions `16x16x1`.
This value is hard-coded in the shader.

So after that fix we get something that looks great!

#### Fixed the clearing

<div class="images-horizontal">
    <div class="image">
        <label>Depth minimums</label>
        <img class="pixelated" width="300vw" src="https://renderling.xyz/uploads/1753562389/5-mins.png" />
    </div>
    <div class="image">
        <label>Depth maximums</label>
        <img class="pixelated" width="300vw" src="https://renderling.xyz/uploads/1753562389/5-maxs.png" />
    </div>
    <div class="image">
        <label>Number of lights</label>
        <img class="pixelated" width="300vw" src="https://renderling.xyz/uploads/1753562389/5-lights.png" />
    </div>
</div>

Let's not get too excited. This scene should have thousands of lights, which means the "Number of lights"
visualization should have some gray area, but it looks binary.

There's only one directional light, so maybe the binning algorithm isn't working for point lights?

### Investigating binning with thousands of lights

When I print out the tiles in the test, I can see that only the one directional light is getting binned.

This leads me to believe it's probably the frustum math that I got wrong.
So I'll create a new test with only one point light and write the AABBs back to the tiling slab to debug.

This is what I get for a tile:

```
LightTile {
    depth_min: 0.998255,
    depth_max: 1.0,
    next_light_index: 1,
    lights_array: Array<crabslab::id::Id<renderling::light::Light>>(244, 2),
    tile_coord: UVec2(
        8,
        6,
    ),
    ndc_aabb: Aabb {
        min: Vec3(
            0.0,
            -0.25,
            0.998255,
        ),
        max: Vec3(
            0.125,
            -0.125,
            1.0,
        ),
    },
    light_aabb: Aabb {
        min: Vec3(
            -1.253029,
            -1.4855967,
            -0.8014438,
        ),
        max: Vec3(
            2.3443582,
            2.1117904,
            2.7959435,
        ),
    },
}    
```

Right off the bat I can tell that `light_aabb` is incorrect.
These AABBs are in normalized device coordinates, so they should always be within `[-1.0, 1.0]` on x and y,
and within `[0.0, 1.0]` on z (depth).

If I add a little more debug info to that struct I can see that the light's bounding sphere looks fine:

```
LightTile {
    depth_min: 0.998255,
    depth_max: 1.0,
    next_light_index: 1,
    lights_array: Array<crabslab::id::Id<renderling::light::Light>>(244, 2),
    tile_coord: UVec2(
        8,
        6,
    ),
    ndc_aabb: Aabb {
        min: Vec3(
            0.0,
            -0.25,
            0.998255,
        ),
        max: Vec3(
            0.125,
            -0.125,
            1.0,
        ),
    },
    light_sphere: BoundingSphere {
        center: Vec3(
            1.1,
            1.0,
            1.1,
        ),
        radius: 4.472136,
    },
    light_sphere_ndc: BoundingSphere {
        center: Vec3(
            0.5456646,
            0.3130969,
            0.9972498,
        ),
        radius: 1.7986937,
    },
    light_aabb: Aabb {
        min: Vec3(
            -1.2530291,
            -1.4855968,
            -0.8014439,
        ),
        max: Vec3(
            2.3443582,
            2.1117907,
            2.7959435,
        ),
    },
}  
```

I can also see the sphere in NDC _seems_ okay, it's at least within the correct range.

The most obviously wrong thing is the `light_aabb`, so I'll check the transformation of
the light's sphere into an AABB.

Actually I think that may be fine.
This light just happens to have an AABB that's bigger than the view frustum.

But it seems like this test is passing and the point light is getting binned, so now I'm
wondering if I misunderstood the problem:

<div class="images-horizontal">
    <div class="image">
        <label>Scene</label>
        <img class="pixelated" width="480vw" src="https://renderling.xyz/uploads/1753574944/1-scene.png" />
    </div>
    <div class="image">
        <label>Bins</label>
        <img class="pixelated" width="480vw" src="https://renderling.xyz/uploads/1753574944/2-lights.png" />
    </div>
</div>

I did rework the way the tile's NDC AABB was calculated, so maybe that fixed it?

...

Indeed! Our thousands-of-lights scene is now, ostensibly, binning properly!


<div class="image">
    <label>Light bins on the thousands-of-lights scene</label>
    <img class="pixelated" width="800vw" src="https://renderling.xyz/uploads/1753575188/5-lights.png" />
</div>

😊😊😊😊😊😊😊
☕☕☕☕☕☕☕

STOKED!

### Frame timing

The frame timing is looking good!

<div class="image">
    <label>Scene rendering time, tiling vs legacy</label>
    <img class="pixelated" width="800vw" src="https://renderling.xyz/uploads/1753582688/frame-time.png" />
</div>

You can see that the time it takes to render one frame increases linearly with the number of lights if
tiling is _not_ used.
When tiling is _on_, apart from the one light case, it doesn't look like it changes at all.

The rendered scene itself is ugly, though, so I'm not sure what happened there, I'll have to do one last
debugging session:

<div class="image">
    <label>The scene rendered using light tiling, obviously buggy</label>
    <img class="pixelated" width="800vw" src="https://renderling.xyz/uploads/1753581800/6-scene.png" />
</div>

It's getting so close now!

## Wed 30 July, 2025

### A little improvement

I haven't have time during the week to work on Renderling, but this evening I had a realization in
the shower...

...the PBR shader's light accumulation iterates over every light in the
lighting array, and the new tiling changes switch which array that is, if
tiling is enabled it'll use the appropriate tile's light list, otherwise it
will use the global list.
But it expects the list to contain valid `Id`s.
An `Id` in Renderling is essentially a "maybe pointer". It's like `Option<u32>`.
The `Id<T>` points to the index on the slab where a `T` is stored.
But it also might not, as it could also be `Id::NONE`, which is a special `Id` that
points to nothing.

Since the light lists are computed on the GPU, which **cannot alloc**, a tile's lights
lists may contain a bunch of `Id::NONE` towards the end, unless tiling found that
the max number of lights are illuminating the tile.

So, long story short, the PBR shader needs to check that the `Id<Light>` that it iterates
over during the radiance accumulation is valid.

With a very small check of `if light_id.is_none() { continue; }`, the scene that gets rendered is now:

<div class="image">
    <label>The scene rendered using light tiling, obviously buggy</label>
    <img class="pixelated" width="800vw" src="https://renderling.xyz/uploads/1753853095/6-scene.png" />
</div>

Ugh! I wish I had that nifty slider-image-comparison widget to show you what it _should_ look like.
Suffice to say that it looks like it is strictly _missing_ lights.

But it looks a lot better!

## Saturday 2 August, 2025

### A small step back

I just realized that my previous fix wasn't quite right.
If the PBR shader encounters a light with an `Id` of `NONE`, it should stop iterating, as there should
be no valid lights following.
So instead of `continue`, it should use `break`.

### Continuing on the last steps

So you can see that there's a band of lights across the middle of the image.
Compare that to the scene without tiling.

<div class="images-horizontal">
    <div class="image">
        <label>The scene rendered using light tiling, obviously buggy</label>
        <img class="pixelated" width="480vw" src="https://renderling.xyz/uploads/1753853095/6-scene.png" />
    </div>
    <div class="image">
        <label>Scene with lighting and shadows, plus point lights, without meshes for the point lights</label>
        <img
            class="pixelated" width="480vw"
            src="https://renderling.xyz/uploads/1745638402/4-after-lights-no-meshes.png"
            alt="a small scene with a couple shapes, including Suzanne, with lighting and extra point lights, but no meshes for the lights" />
    </div>
</div>

Like I said before, it looks like it's strictly _missing_ lights.
But other than that, it looks like it's working for that narrow band in the middle.

To debug this situation, we can first take a look at the tiles.
I'll pick a tile that should have some lights, but doesn't, and we'll read that
off the slab, along with its computed light array.
Hopefully that will "illuminate" the problem ;)

I'm picking the point `1917x979`, which should correspond to tile `(119,61)`:

<div class="image">
    <label>The chosen tile, notice that there should be a large red point light illuminating this tile</label>
    <img class="pixelated" width="800vw" src="https://renderling.xyz/uploads/1754085552/Screenshot_2025-08-02_at_9.58.57AM.png" />
</div>

There should be a pretty bright red light illuminating that tile, so let's read
it and see what we get.

```
tile: LightTile {
    depth_min: 0.9533538,
    depth_max: 0.9547121,
    next_light_index: 6,
    lights_array: Array<crabslab::id::Id<renderling::light::Light>>(339753, 32),
}
lights_ids: [
    Id<renderling::light::Light>(27),
    Id<renderling::light::Light>(4684),
    Id<renderling::light::Light>(13044),
    Id<renderling::light::Light>(19952),
    Id<renderling::light::Light>(966),
    Id<renderling::light::Light>(19468),
    Id<renderling::light::Light>(null),
    Id<renderling::light::Light>(null),
    Id<renderling::light::Light>(null),
    Id<renderling::light::Light>(null),
    Id<renderling::light::Light>(null),
    Id<renderling::light::Light>(null),
    Id<renderling::light::Light>(null),
    Id<renderling::light::Light>(null),
    Id<renderling::light::Light>(null),
    Id<renderling::light::Light>(null),
    Id<renderling::light::Light>(null),
    Id<renderling::light::Light>(null),
    Id<renderling::light::Light>(null),
    Id<renderling::light::Light>(null),
    Id<renderling::light::Light>(null),
    Id<renderling::light::Light>(null),
    Id<renderling::light::Light>(null),
    Id<renderling::light::Light>(null),
    Id<renderling::light::Light>(null),
    Id<renderling::light::Light>(null),
    Id<renderling::light::Light>(null),
    Id<renderling::light::Light>(null),
    Id<renderling::light::Light>(null),
    Id<renderling::light::Light>(null),
    Id<renderling::light::Light>(null),
    Id<renderling::light::Light>(null),
]
Id<renderling::light::Light>(27): Light {
    light_type: Directional,
    index: 19,
    transform_id: Id<renderling::transform::Transform>(9),
    shadow_map_desc_id: Id<renderling::light::ShadowMapDescriptor>(59),
}
details: DirectionalLightDescriptor {
    direction: Vec3(
        0.0,
        0.0,
        -1.0,
    ),
    color: Vec4(
        0.5569815,
        0.64621955,
        1.0,
        1.0,
    ),
    intensity: 2.0,
}
Id<renderling::light::Light>(4684): Light {
    light_type: Point,
    index: 4676,
    transform_id: Id<renderling::transform::Transform>(4666),
    shadow_map_desc_id: Id<renderling::light::ShadowMapDescriptor>(null),
}
details: PointLightDescriptor {
    position: Vec3(
        112.692276,
        37.322506,
        33.508026,
    ),
    color: Vec4(
        0.7668487,
        0.7334805,
        0.2654935,
        1.0,
    ),
    intensity: 55.458416,
}
Id<renderling::light::Light>(13044): Light {
    light_type: Point,
    index: 13036,
    transform_id: Id<renderling::transform::Transform>(13026),
    shadow_map_desc_id: Id<renderling::light::ShadowMapDescriptor>(null),
}
details: PointLightDescriptor {
    position: Vec3(
        77.29234,
        1.0830231,
        -32.90143,
    ),
    color: Vec4(
        0.27049473,
        0.31844544,
        0.7464798,
        1.0,
    ),
    intensity: 85.284355,
}
Id<renderling::light::Light>(19952): Light {
    light_type: Point,
    index: 19944,
    transform_id: Id<renderling::transform::Transform>(19934),
    shadow_map_desc_id: Id<renderling::light::ShadowMapDescriptor>(null),
}
details: PointLightDescriptor {
    position: Vec3(
        101.35216,
        8.859904,
        -23.04837,
    ),
    color: Vec4(
        0.9081591,
        0.14542232,
        0.84428316,
        1.0,
    ),
    intensity: 65.89107,
}
Id<renderling::light::Light>(966): Light {
    light_type: Point,
    index: 958,
    transform_id: Id<renderling::transform::Transform>(948),
    shadow_map_desc_id: Id<renderling::light::ShadowMapDescriptor>(null),
}
details: PointLightDescriptor {
    position: Vec3(
        98.49878,
        1.3413525,
        -4.739517,
    ),
    color: Vec4(
        0.3320842,
        0.15595555,
        0.001793537,
        1.0,
    ),
    intensity: 72.14496,
}
Id<renderling::light::Light>(19468): Light {
    light_type: Point,
    index: 19460,
    transform_id: Id<renderling::transform::Transform>(19450),
    shadow_map_desc_id: Id<renderling::light::ShadowMapDescriptor>(null),
}
details: PointLightDescriptor {
    position: Vec3(
        117.828125,
        41.12835,
        41.137863,
    ),
    color: Vec4(
        0.57746273,
        0.372337,
        0.27789938,
        1.0,
    ),
    intensity: 52.44305,
}
```

## Sunday 3 August, 2025

As far as I can tell, that tile has plenty of lighting.
And it seems at least that the directional light is being evaluated for all the tiles, that's interesting.

Maybe this tile isn't being selected by this fragment?

Hrm, it's selecting the correct tile.

Let's look at another tile - one further away from the "blessed band".

I'll choose this one, since it should be affected by lights in the -X, +Y, -Z quadrant:

<div class="image">
    <label>The next chosen tile, notice that there should be a large orange point light illuminating this tile</label>
    <img class="pixelated" width="800vw" src="https://renderling.xyz/uploads/1754166914/Screenshot_2025-08-03_at_8.34.58AM.png" />
</div>

It has many more lights illuminating it:

```
tile: LightTile {
    depth_min: 0.9395258,
    depth_max: 0.94081247,
    next_light_index: 29,
    lights_array: Array<crabslab::id::Id<renderling::light::Light>>(449449, 32),
}
lights_ids: [
    Id<renderling::light::Light>(27),
    Id<renderling::light::Light>(2462),
    Id<renderling::light::Light>(2484),
    Id<renderling::light::Light>(2902),
    Id<renderling::light::Light>(3232),
    Id<renderling::light::Light>(2990),
    Id<renderling::light::Light>(12428),
    Id<renderling::light::Light>(4310),
    Id<renderling::light::Light>(6136),
    Id<renderling::light::Light>(7148),
    Id<renderling::light::Light>(8028),
    Id<renderling::light::Light>(9788),
    Id<renderling::light::Light>(10668),
    Id<renderling::light::Light>(18082),
    Id<renderling::light::Light>(18214),
    Id<renderling::light::Light>(13880),
    Id<renderling::light::Light>(15310),
    Id<renderling::light::Light>(16674),
    Id<renderling::light::Light>(16938),
    Id<renderling::light::Light>(14188),
    Id<renderling::light::Light>(14738),
    Id<renderling::light::Light>(17004),
    Id<renderling::light::Light>(10228),
    Id<renderling::light::Light>(19314),
    Id<renderling::light::Light>(18720),
    Id<renderling::light::Light>(18984),
    Id<renderling::light::Light>(15970),
    Id<renderling::light::Light>(15860),
    Id<renderling::light::Light>(21778),
    Id<renderling::light::Light>(null),
    Id<renderling::light::Light>(null),
    Id<renderling::light::Light>(null),
]
Id<renderling::light::Light>(27): Light {
    light_type: Directional,
    index: 19,
    transform_id: Id<renderling::transform::Transform>(9),
    shadow_map_desc_id: Id<renderling::light::ShadowMapDescriptor>(59),
}
details: DirectionalLightDescriptor {
    direction: Vec3(
        0.0,
        0.0,
        -1.0,
    ),
    color: Vec4(
        0.5569815,
        0.64621955,
        1.0,
        1.0,
    ),
    intensity: 2.0,
}
Id<renderling::light::Light>(2462): Light {
    light_type: Point,
    index: 2454,
    transform_id: Id<renderling::transform::Transform>(2444),
    shadow_map_desc_id: Id<renderling::light::ShadowMapDescriptor>(null),
}
details: PointLightDescriptor {
    position: Vec3(
        -92.63359,
        42.473137,
        85.84752,
    ),
    color: Vec4(
        0.8666822,
        0.56193554,
        0.008014433,
        1.0,
    ),
    intensity: 93.190994,
}
Id<renderling::light::Light>(2484): Light {
    light_type: Point,
    index: 2476,
    transform_id: Id<renderling::transform::Transform>(2466),
    shadow_map_desc_id: Id<renderling::light::ShadowMapDescriptor>(null),
}
details: PointLightDescriptor {
    position: Vec3(
        -81.83816,
        39.18519,
        88.81053,
    ),
    color: Vec4(
        0.20394476,
        0.62335926,
        0.62762266,
        1.0,
    ),
    intensity: 13.299932,
}
Id<renderling::light::Light>(2902): Light {
    light_type: Point,
    index: 2894,
    transform_id: Id<renderling::transform::Transform>(2884),
    shadow_map_desc_id: Id<renderling::light::ShadowMapDescriptor>(null),
}
details: PointLightDescriptor {
    position: Vec3(
        -24.61863,
        66.47632,
        95.26399,
    ),
    color: Vec4(
        0.7518719,
        0.4373035,
        0.90187305,
        1.0,
    ),
    intensity: 57.083206,
}
Id<renderling::light::Light>(3232): Light {
    light_type: Point,
    index: 3224,
    transform_id: Id<renderling::transform::Transform>(3214),
    shadow_map_desc_id: Id<renderling::light::ShadowMapDescriptor>(null),
}
details: PointLightDescriptor {
    position: Vec3(
        -77.335526,
        42.237896,
        79.17302,
    ),
    color: Vec4(
        0.32873732,
        0.46097255,
        0.25026292,
        1.0,
    ),
    intensity: 32.47139,
}
Id<renderling::light::Light>(2990): Light {
    light_type: Point,
    index: 2982,
    transform_id: Id<renderling::transform::Transform>(2972),
    shadow_map_desc_id: Id<renderling::light::ShadowMapDescriptor>(null),
}
details: PointLightDescriptor {
    position: Vec3(
        -94.03255,
        24.309181,
        96.74237,
    ),
    color: Vec4(
        0.62377965,
        0.18438692,
        0.4720049,
        1.0,
    ),
    intensity: 79.36237,
}
Id<renderling::light::Light>(12428): Light {
    light_type: Point,
    index: 12420,
    transform_id: Id<renderling::transform::Transform>(12410),
    shadow_map_desc_id: Id<renderling::light::ShadowMapDescriptor>(null),
}
details: PointLightDescriptor {
    position: Vec3(
        4.4616394,
        60.307003,
        118.59419,
    ),
    color: Vec4(
        0.12806311,
        0.53484213,
        0.21150446,
        1.0,
    ),
    intensity: 32.674458,
}
Id<renderling::light::Light>(4310): Light {
    light_type: Point,
    index: 4302,
    transform_id: Id<renderling::transform::Transform>(4292),
    shadow_map_desc_id: Id<renderling::light::ShadowMapDescriptor>(null),
}
details: PointLightDescriptor {
    position: Vec3(
        -65.93443,
        52.837452,
        89.24689,
    ),
    color: Vec4(
        0.26129723,
        0.20125352,
        0.07756548,
        1.0,
    ),
    intensity: 63.60116,
}
Id<renderling::light::Light>(6136): Light {
    light_type: Point,
    index: 6128,
    transform_id: Id<renderling::transform::Transform>(6118),
    shadow_map_desc_id: Id<renderling::light::ShadowMapDescriptor>(null),
}
details: PointLightDescriptor {
    position: Vec3(
        -76.65203,
        40.773567,
        92.28496,
    ),
    color: Vec4(
        0.3980166,
        0.80558443,
        0.568609,
        1.0,
    ),
    intensity: 35.141464,
}
Id<renderling::light::Light>(7148): Light {
    light_type: Point,
    index: 7140,
    transform_id: Id<renderling::transform::Transform>(7130),
    shadow_map_desc_id: Id<renderling::light::ShadowMapDescriptor>(null),
}
details: PointLightDescriptor {
    position: Vec3(
        -34.733917,
        58.189674,
        96.49277,
    ),
    color: Vec4(
        0.8524464,
        0.18924342,
        0.09058161,
        1.0,
    ),
    intensity: 27.794355,
}
Id<renderling::light::Light>(8028): Light {
    light_type: Point,
    index: 8020,
    transform_id: Id<renderling::transform::Transform>(8010),
    shadow_map_desc_id: Id<renderling::light::ShadowMapDescriptor>(null),
}
details: PointLightDescriptor {
    position: Vec3(
        -90.89901,
        35.59253,
        98.67053,
    ),
    color: Vec4(
        0.21548326,
        0.2987796,
        0.01059004,
        1.0,
    ),
    intensity: 82.311455,
}
Id<renderling::light::Light>(9788): Light {
    light_type: Point,
    index: 9780,
    transform_id: Id<renderling::transform::Transform>(9770),
    shadow_map_desc_id: Id<renderling::light::ShadowMapDescriptor>(null),
}
details: PointLightDescriptor {
    position: Vec3(
        -118.82065,
        4.7073503,
        64.45058,
    ),
    color: Vec4(
        0.496927,
        0.114739984,
        0.45658094,
        1.0,
    ),
    intensity: 72.02357,
}
Id<renderling::light::Light>(10668): Light {
    light_type: Point,
    index: 10660,
    transform_id: Id<renderling::transform::Transform>(10650),
    shadow_map_desc_id: Id<renderling::light::ShadowMapDescriptor>(null),
}
details: PointLightDescriptor {
    position: Vec3(
        -29.81083,
        54.179108,
        106.95091,
    ),
    color: Vec4(
        0.2608614,
        0.25088722,
        0.2754782,
        1.0,
    ),
    intensity: 73.07852,
}
Id<renderling::light::Light>(18082): Light {
    light_type: Point,
    index: 18074,
    transform_id: Id<renderling::transform::Transform>(18064),
    shadow_map_desc_id: Id<renderling::light::ShadowMapDescriptor>(null),
}
details: PointLightDescriptor {
    position: Vec3(
        -24.39624,
        44.871983,
        119.648895,
    ),
    color: Vec4(
        0.58111584,
        0.7989411,
        0.17732547,
        1.0,
    ),
    intensity: 81.52727,
}
Id<renderling::light::Light>(18214): Light {
    light_type: Point,
    index: 18206,
    transform_id: Id<renderling::transform::Transform>(18196),
    shadow_map_desc_id: Id<renderling::light::ShadowMapDescriptor>(null),
}
details: PointLightDescriptor {
    position: Vec3(
        -90.108246,
        28.47501,
        101.11792,
    ),
    color: Vec4(
        0.30298874,
        0.022064472,
        0.66243684,
        1.0,
    ),
    intensity: 86.666176,
}
Id<renderling::light::Light>(13880): Light {
    light_type: Point,
    index: 13872,
    transform_id: Id<renderling::transform::Transform>(13862),
    shadow_map_desc_id: Id<renderling::light::ShadowMapDescriptor>(null),
}
details: PointLightDescriptor {
    position: Vec3(
        -73.776535,
        21.51609,
        94.31854,
    ),
    color: Vec4(
        0.26402467,
        0.5380622,
        0.8576432,
        1.0,
    ),
    intensity: 61.12759,
}
Id<renderling::light::Light>(15310): Light {
    light_type: Point,
    index: 15302,
    transform_id: Id<renderling::transform::Transform>(15292),
    shadow_map_desc_id: Id<renderling::light::ShadowMapDescriptor>(null),
}
details: PointLightDescriptor {
    position: Vec3(
        -88.47996,
        37.740273,
        81.930115,
    ),
    color: Vec4(
        0.95636755,
        0.23839176,
        0.07199448,
        1.0,
    ),
    intensity: 38.299305,
}
Id<renderling::light::Light>(16674): Light {
    light_type: Point,
    index: 16666,
    transform_id: Id<renderling::transform::Transform>(16656),
    shadow_map_desc_id: Id<renderling::light::ShadowMapDescriptor>(null),
}
details: PointLightDescriptor {
    position: Vec3(
        -87.211006,
        32.31549,
        66.26398,
    ),
    color: Vec4(
        0.9328823,
        0.12743318,
        0.42688933,
        1.0,
    ),
    intensity: 55.027626,
}
Id<renderling::light::Light>(16938): Light {
    light_type: Point,
    index: 16930,
    transform_id: Id<renderling::transform::Transform>(16920),
    shadow_map_desc_id: Id<renderling::light::ShadowMapDescriptor>(null),
}
details: PointLightDescriptor {
    position: Vec3(
        -24.065544,
        68.6632,
        104.902725,
    ),
    color: Vec4(
        0.80462956,
        0.4366882,
        0.27665752,
        1.0,
    ),
    intensity: 35.60955,
}
Id<renderling::light::Light>(14188): Light {
    light_type: Point,
    index: 14180,
    transform_id: Id<renderling::transform::Transform>(14170),
    shadow_map_desc_id: Id<renderling::light::ShadowMapDescriptor>(null),
}
details: PointLightDescriptor {
    position: Vec3(
        -91.12869,
        25.659275,
        89.22908,
    ),
    color: Vec4(
        0.120221116,
        0.6146752,
        0.3619943,
        1.0,
    ),
    intensity: 99.014786,
}
Id<renderling::light::Light>(14738): Light {
    light_type: Point,
    index: 14730,
    transform_id: Id<renderling::transform::Transform>(14720),
    shadow_map_desc_id: Id<renderling::light::ShadowMapDescriptor>(null),
}
details: PointLightDescriptor {
    position: Vec3(
        -71.20869,
        33.790966,
        85.73744,
    ),
    color: Vec4(
        0.0076795463,
        0.62384826,
        0.2103971,
        1.0,
    ),
    intensity: 61.760628,
}
Id<renderling::light::Light>(17004): Light {
    light_type: Point,
    index: 16996,
    transform_id: Id<renderling::transform::Transform>(16986),
    shadow_map_desc_id: Id<renderling::light::ShadowMapDescriptor>(null),
}
details: PointLightDescriptor {
    position: Vec3(
        -73.52121,
        39.84316,
        83.56464,
    ),
    color: Vec4(
        0.7455217,
        0.06382919,
        0.51204777,
        1.0,
    ),
    intensity: 70.79098,
}
Id<renderling::light::Light>(10228): Light {
    light_type: Point,
    index: 10220,
    transform_id: Id<renderling::transform::Transform>(10210),
    shadow_map_desc_id: Id<renderling::light::ShadowMapDescriptor>(null),
}
details: PointLightDescriptor {
    position: Vec3(
        -50.901886,
        47.145916,
        111.39549,
    ),
    color: Vec4(
        0.54008365,
        0.81548566,
        0.5111073,
        1.0,
    ),
    intensity: 46.961178,
}
Id<renderling::light::Light>(19314): Light {
    light_type: Point,
    index: 19306,
    transform_id: Id<renderling::transform::Transform>(19296),
    shadow_map_desc_id: Id<renderling::light::ShadowMapDescriptor>(null),
}
details: PointLightDescriptor {
    position: Vec3(
        -99.66124,
        28.869125,
        69.396515,
    ),
    color: Vec4(
        0.7421489,
        0.8242992,
        0.8321162,
        1.0,
    ),
    intensity: 79.120445,
}
Id<renderling::light::Light>(18720): Light {
    light_type: Point,
    index: 18712,
    transform_id: Id<renderling::transform::Transform>(18702),
    shadow_map_desc_id: Id<renderling::light::ShadowMapDescriptor>(null),
}
details: PointLightDescriptor {
    position: Vec3(
        -67.99999,
        45.229263,
        78.93985,
    ),
    color: Vec4(
        0.72421217,
        0.7909413,
        0.5956649,
        1.0,
    ),
    intensity: 93.47528,
}
Id<renderling::light::Light>(18984): Light {
    light_type: Point,
    index: 18976,
    transform_id: Id<renderling::transform::Transform>(18966),
    shadow_map_desc_id: Id<renderling::light::ShadowMapDescriptor>(null),
}
details: PointLightDescriptor {
    position: Vec3(
        -113.1403,
        4.0609326,
        74.15787,
    ),
    color: Vec4(
        0.49988353,
        0.86962765,
        0.34533152,
        1.0,
    ),
    intensity: 37.66789,
}
Id<renderling::light::Light>(15970): Light {
    light_type: Point,
    index: 15962,
    transform_id: Id<renderling::transform::Transform>(15952),
    shadow_map_desc_id: Id<renderling::light::ShadowMapDescriptor>(null),
}
details: PointLightDescriptor {
    position: Vec3(
        -42.14424,
        57.334972,
        94.4227,
    ),
    color: Vec4(
        0.87706107,
        0.19849941,
        0.89152145,
        1.0,
    ),
    intensity: 99.22268,
}
Id<renderling::light::Light>(15860): Light {
    light_type: Point,
    index: 15852,
    transform_id: Id<renderling::transform::Transform>(15842),
    shadow_map_desc_id: Id<renderling::light::ShadowMapDescriptor>(null),
}
details: PointLightDescriptor {
    position: Vec3(
        -107.09671,
        6.606737,
        81.26341,
    ),
    color: Vec4(
        0.57034963,
        0.13842575,
        0.7675047,
        1.0,
    ),
    intensity: 73.670906,
}
Id<renderling::light::Light>(21778): Light {
    light_type: Point,
    index: 21770,
    transform_id: Id<renderling::transform::Transform>(21760),
    shadow_map_desc_id: Id<renderling::light::ShadowMapDescriptor>(null),
}
details: PointLightDescriptor {
    position: Vec3(
        -52.096153,
        59.919052,
        108.22235,
    ),
    color: Vec4(
        0.9006374,
        0.49105757,
        0.44246408,
        1.0,
    ),
    intensity: 27.273262,
}    
```

None of these lights are in the -X,+Y,-Z quadrant.
I think that means the wrong lights are being selected.
The light from these point lights is likely being attenuated into nothing by the PBR shader.

This tile has a calculated NDC min and max of
```
(
    Vec2(
        -0.6625,
        0.15277779,
    ),
    Vec2(
        -0.65,
        0.16666663,
    ),
)
```

I think that's it. The tile lives on the bottom-left half of the viewport, so its NDC min and max should be
negative in both components.

I can write a unit test for this.

Oof! I got it.

I forgot to flip the Y axis when converting from "tile space" to NDC.

After fixing that, the scene is much better!

<div class="image">
    <label>The scene, with tiling!</label>
    <img class="pixelated" width="880vw" src="https://renderling.xyz/uploads/1754175040/6-scene.png" />
</div>

There's obviously some tweaking that's needed, but this is pretty much the whole idea!

My guess is that I need to adjust the radius of illumination of the lights to include more lights.

### Radius of illumination

I'm making the minimum illuminance, used to determine the radius of illumination of a light, as a
configurable CPU-side value.

Let's see what happens with different numbers.

<div class="images-horizontal">
  <div class="image">
    <label>Tiling with minimum illuminance `0.02`</label>
    <img class="pixelated" width="500vw" src="https://renderling.xyz/uploads/1754178781/6-scene-illuminance-0.02.png" />
  </div>
  <div class="image">
    <label>Tiling with minimum illuminance `0.04`</label>
    <img class="pixelated" width="500vw" src="https://renderling.xyz/uploads/1754178781/6-scene-illuminance-0.04.png" />
  </div>
  <div class="image">
    <label>Tiling with minimum illuminance `0.08`</label>
    <img class="pixelated" width="500vw" src="https://renderling.xyz/uploads/1754178781/6-scene-illuminance-0.08.png" />
  </div>
  <div class="image">
    <label>Tiling with minimum illuminance `0.16`</label>
    <img class="pixelated" width="500vw" src="https://renderling.xyz/uploads/1754178781/6-scene-illuminance-0.16.png" />
  </div>
  <div class="image">
    <label>Tiling with minimum illuminance `0.25`</label>
    <img class="pixelated" width="500vw" src="https://renderling.xyz/uploads/1754178781/6-scene-illuminance-0.25.png" />
  </div>
  <div class="image">
    <label>Tiling with minimum illuminance `0.5`</label>
    <img class="pixelated" width="500vw" src="https://renderling.xyz/uploads/1754178781/6-scene-illuminance-0.5.png" />
  </div>
</div>

## Light list ordering - Saturday 9 August, 2025

Speaking of minimum illuminance - the concept itself is an optimization to
get around the fact we can't store all the lights in each tile. We have
limited space and we need a way to filter out the lights and only apply
those that have a significant effect on the tile.

But there's an obvious bug here.
When there are lots of lights affecting the tile, the tile's light array slots
fill up quickly.
Exacerbating the situation is the fact that getting a slot in the tile's light
array is a first-come, first-serve operation.
We can never be sure of the shader invocation order, which means some lights
that should be included, are not.
That makes this whole system temporally unstable.
In one frame you may get the "strongest" lights in the array, yet in the next
the tile may be filled with lots of "weak" illuminators. 

So how do we solve this problem?

My intuition is to get rid of the radius of illumination check and instead
somehow order _all_ the lights by `intensity/distance^2`, and then take the top
*`N`* "strongest".

But ordering in a parallel shader is really tricky. I honestly don't know
what that would look like.
 
I've found an interesting article about [GPU hash tables made from "slab lists"](https://arxiv.org/pdf/1710.11246).
Maybe what I should try doing is something incredibly naive, just to see if
it fixes the instability issue, then spend some time making it better.

You know what they say - [make it work, make it right, make it
fast](/articles/adages.html#make-it-work-make-it-right-make-it-fast)

## Do the simplest thing - Sunday 10 August, 2025

So I decided that the simplest thing to do would be to use a spin lock to
replace the weakest light in the case that the light list is full.

But of course it's always more complicated than that...

After writing the spin lock and updating my shader, I get a `naga` error
about atomic upgrades, which means my shader isn't being translated
correctly.

Funny thing is, **I wrote the atomic upgrade code in `naga`**. Lol.
So this is _my bug_.
Or maybe `naga` is operating correctly.
Either way I need to create a minimally reproducible test case.

### Naga bug in the Metal backend

I've run into a `naga` bug.

```
thread 'light::cpu::test::tiling_e2e_sanity' panicked at /Users/schell/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/naga-24.0.0/src/back/msl/writer.rs:2255:17:
internal error: entered unreachable code
```

So as a first step, I'm updating `wgpu`, `naga` and `metal`.

That causes a cascade of changes...

I opened a bug report in `naga` for the error in the metal backend [here](https://github.com/gfx-rs/wgpu/issues/8072).

### Backtracking 

For now, I'm going to park this work, backtrack a bit and see if simply increasing the tile's light array size can help.

It looks like it doesn't much matter what the minimum illuminance value is.
All the renders look pretty much the same:

<div class="image">
    <label>After backtracking, slightly different method for determining illumination</label>
    <img class="pixelated" width="980vw" src="https://renderling.xyz/uploads/1754887013/6-scene-illuminance-0.04.png" />
</div>

It seems like there's some rather uniform unwanted artifacts here.
When I change the length of the tile's light list I can see the problem, starkly:

<div class="images-horizontal">
    <div class="image">
        <label>Bin of 32</label>
        <img class="pixelated" width="155vw" src="https://renderling.xyz/uploads/1754888366/6-scene-min-lights-32.png" />
    </div>
    <div class="image">
        <label>Bin of 64</label>
        <img class="pixelated" width="155vw" src="https://renderling.xyz/uploads/1754888366/6-scene-min-lights-64.png" />
    </div>
    <div class="image">
        <label>Bin of 128</label>
        <img class="pixelated" width="155vw" src="https://renderling.xyz/uploads/1754888366/6-scene-min-lights-128.png" />
    </div>
      <div class="image">
        <label>Bin of 256</label>
        <img class="pixelated" width="155vw" src="https://renderling.xyz/uploads/1754888366/6-scene-min-lights-256.png" />
    </div>
            <div class="image">
        <label>Bin of 512</label>
        <img class="pixelated" width="155vw" src="https://renderling.xyz/uploads/1754888366/6-scene-min-lights-512.png" />
    </div>
    <div class="image">
        <label>Bin of 1024</label>
        <img class="pixelated" width="155vw" src="https://renderling.xyz/uploads/1754888366/6-scene-min-lights-1024.png" />
    </div>
</div>

I think this makes it pretty obvious that there should be some sort of ordering or priority to the lights that get put in
the bin.

Or maybe we need to find a balance between minimum illuminance and the size of the bin.

## Comparing tiles - Tue 12 August, 2025

I think the problem we're seeing is what I mentioned earlier. 
When there are a lot of lights that illuminate a tile, slots in the the tile's
light array fill up.
Reserving a slot in the array is atomic, but first-come-first-serve, and the
order of shader invocations is non-deterministic.
That means that each tile could contain a different set of lights each
frame when there are lots of lights.

So I think this problem is causing the artfacts we see in the pictures above.
My guess is that the dark tiles don't contain the directional light that is the
primary illuminator in the scene.

## Oh shoot, coordinates!

But before I go too far down that rabbit hole, I have a realization. 

Let's reiterate the rough steps in the "light binning" algo:

1. Compute the tile's frustum in NDC coordinates.
2. Compute each light's position in NDC coordinates.
3. Compute the light's radius of illumination using the light intensity and the minimum illuminance passed in from the CPU.
4. Determine if the light intersects the frustum and add it to the bin of lights.

Maybe you spotted the problem here.
I didn't, until now.

> Compute the light's radius of illumination using the light intensity and the minimum illuminance passed in from the CPU.

There it is.
Minimum illuminance is in **lux**!
Lux is Lumens per **meter** squared, and I'm using it in conjuction with the
frustum and light position in NDC coords!

NDC coords are unitless and completely in terms of the view frustum.
Meters are meters.
So here we're not comparing apples to apples.

The  radius of illumination is in meters, but the distance from the light to the
frustum is in NDC.
This has resulted in almost every light passing the comparison test and getting
binned.

This is the real bug here, though this bug greatly exaggerates the effects of
the bug I _thought_ was the problem.

I think we can live with the other bug, and solve it by tuning the values for
the minimum illuminance and the tile's bin size, and maybe the tile
size.

### So much better

After switching to calculating everything in world coords, it looks much better.
You can still see that at a low bin size we still get a good amount of 
artifacts, but it's not as bad.

#### Bin size 32

<div class="images-horizontal">
  <div class="image">
    <label>min illuminance 0.1</label>
    <img class="pixelated" width="310vw" src="https://renderling.xyz/uploads/1754951785/6-scene-32-0-0.1.png" />
  </div>
  <div class="image">
    <label>min illuminance 0.2</label>
    <img class="pixelated" width="310vw" src="https://renderling.xyz/uploads/1754951785/6-scene-32-1-0.2.png" />
  </div>
  <div class="image">
    <label>min illuminance 0.5</label>
    <img class="pixelated" width="310vw" src="https://renderling.xyz/uploads/1754951785/6-scene-32-2-0.5.png" />
  </div>
  <div class="image">
    <label>min illuminance 1</label>
    <img class="pixelated" width="310vw" src="https://renderling.xyz/uploads/1754951785/6-scene-32-3-1.png" />
  </div>
  <div class="image">
    <label>min illuminance 2</label>
    <img class="pixelated" width="310vw" src="https://renderling.xyz/uploads/1754951785/6-scene-32-4-2.png" />
  </div>
  <div class="image">
    <label>min illuminance 5</label>
    <img class="pixelated" width="310vw" src="https://renderling.xyz/uploads/1754951785/6-scene-32-5-5.png" />
  </div>
</div>

#### Bin size 64

<div class="images-horizontal">
  <div class="image">
    <label>min illuminance 0.1</label>
    <img class="pixelated" width="310vw" src="https://renderling.xyz/uploads/1754953799/6-scene-64-0-0.1.png" />
  </div>
  <div class="image">
    <label>min illuminance 0.2</label>
    <img class="pixelated" width="310vw" src="https://renderling.xyz/uploads/1754953799/6-scene-64-1-0.2.png" />
  </div>
  <div class="image">
    <label>min illuminance 0.5</label>
    <img class="pixelated" width="310vw" src="https://renderling.xyz/uploads/1754953799/6-scene-64-2-0.5.png" />
  </div>
  <div class="image">
    <label>min illuminance 1</label>
    <img class="pixelated" width="310vw" src="https://renderling.xyz/uploads/1754953799/6-scene-64-3-1.png" />
  </div>
  <div class="image">
    <label>min illuminance 2</label>
    <img class="pixelated" width="310vw" src="https://renderling.xyz/uploads/1754953799/6-scene-64-4-2.png" />
  </div>
  <div class="image">
    <label>min illuminance 5</label>
    <img class="pixelated" width="310vw" src="https://renderling.xyz/uploads/1754953799/6-scene-64-5-5.png" />
  </div>
</div>

#### Bin size 128

<div class="images-horizontal">
  <div class="image">
    <label>min illuminance 0.1</label>
    <img class="pixelated" width="310vw" src="https://renderling.xyz/uploads/1754953978/6-scene-128-0-0.1.png" />
</div>
  <div class="image">
    <label>min illuminance 0.2</label>
    <img class="pixelated" width="310vw" src="https://renderling.xyz/uploads/1754953978/6-scene-128-1-0.2.png" />
</div>
  <div class="image">
    <label>min illuminance 0.5</label>
    <img class="pixelated" width="310vw" src="https://renderling.xyz/uploads/1754953978/6-scene-128-2-0.5.png" />
</div>
  <div class="image">
    <label>min illuminance 1</label>
    <img class="pixelated" width="310vw" src="https://renderling.xyz/uploads/1754953978/6-scene-128-3-1.png" />
</div>
  <div class="image">
    <label>min illuminance 2</label>
    <img class="pixelated" width="310vw" src="https://renderling.xyz/uploads/1754953978/6-scene-128-4-2.png" />
</div>
  <div class="image">
    <label>min illuminance 5</label>
    <img class="pixelated" width="310vw" src="https://renderling.xyz/uploads/1754953978/6-scene-128-5-5.png" />
</div>
</div>

Here you can see that a tile light bin size of 128 and minimum illuminance of
0.1 looks good, so I think I can consider this bug squashed!

Now let's see how a bin size of 128 affects runtime performance...

<div class="image">
    <label>Frame timing with bin size 128</label>
    <img class="pixelated" width="980vw" src="https://renderling.xyz/uploads/1754954317/frame-time.png" />
</div>

Wow! It's still well over 100fps.

### Tile size

The last bit of business before considering this feature "done", is to make the
tile size configurable.

...

Nothing to report, there.
Using a smaller tile size makes the radius of illumination very apparent for
largish values of minimum illuminance: 

<div class="image">
    <label>Tile size 4, bin size 16, min illuminance 1.0</label>
    <img class="pixelated" width="980vw" src="https://renderling.xyz/uploads/1755041420/6-scene-4-16-lights-3-1-min-lux.png" />
</div>

But with that small a tile size, tiling doesn't pay off until we have about 700 lights in the scene:

<div class="image">
    <label>Tile size 4, bin size 16, min illuminance 1.0</label>
    <img class="pixelated" width="980vw" src="https://renderling.xyz/uploads/1755041593/frame-time-4-16-lights-3-1-min-lux.png" />
</div>

A better compromise, at least for this scene, seems to be a tile size of 16, bin size of 128 and minimum illuminance 0.1:

<div class="image">
    <label>Tile size 16, bin size 128, min illuminance 0.1</label>
    <img class="pixelated" width="980vw" src="https://renderling.xyz/uploads/1755042142/6-scene-16-128-lights-1-0.1-min-lux.png" />
</div>

<div class="image">
    <label>Tile size 16, bin size 128, min illuminance 0.1</label>
    <img class="pixelated" width="980vw" src="https://renderling.xyz/uploads/1755042209/frame-time-16-128-lights-1-0.1-min-lux.png" />
</div>

Either way, these parameters can be fine-tuned at will, so any scene can be accommodated.

# That's a wrap!

That's it for the build out!
Thanks for reading this far.

The [light tiling PR](https://github.com/schell/renderling/pull/163) is a
whopper, weighing in at 5000 lines, most of it adding tests and moving
things.

It's been ongoing for about 5 months now!
This feature is the capstone on the projects 2024 NLNet funding, so I'm really,
really glad they decided to extend the runway by a couple months.
It feels good to get this done!

Now on to planning the next year of work!

🙇🙇🙇🙇🙇
