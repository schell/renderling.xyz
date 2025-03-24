---
title: Light Tiling, Live!
---
_Following along with Renderling's initial implementation of light tiling_

# Introduction to light tiling - Sat 22 Mar 2025

I'm finally starting out on the feature that I created Renderling for - light tiling.
This will be the capstone on Renderling's "Forward+" approach to rendering.

The state of the art was introduced by Takahiro Harada, Jay McKee, and Jason Yang in their paper
["Forward+: Bringing Deferred Lighting to the Next Level"](https://takahiroharada.wordpress.com/wp-content/uploads/2015/04/forward_plus.pdf)...
...in 2012!
At the time of this writing, that's 13 years ago. Time flies!

When I read that paper I saw this screenshot, and was really impressed:

<div class="image">
    <label>A screenshot from the AMD Leo demo using Forward+</label>
    <img
        src="https://renderling.xyz/uploads/1742612068/Screenshot_2025-03-22_at_3.53.46PM.png"
        alt="A screenshot from the AMD Leo demo using Forward+" />
</div>

You can still see that demo [here, on youtube](https://www.youtube.com/watch?v=C6TUVsmNUKI).

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

So what we can do is divide the screen into small tiles, calculate which lights illuminate the tile,
store them in a list and then traverse _that list_ for the fragments in the tile, greatly reducing the
number of lights we need to visit per fragment.

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
map, and then we'll use some occlusion culling to only "redraw" the meshlets
that are in the scene and visible. We'll also compute the light tiles. Then
we'll do the redraw. During that redraw is when we'll do shading and MSAA will
resolve. So I _think_ that's all fine.

I could even implement light tiling _without the occlusion culling_ and it
would probably only bump the render time up a little. We'll see.

Anyway - let's do the depth pre-pass.

## Depth pre-pass work

I think what I'll do in order to get cracking as soon as possible (in pursuit
of [the 20/80 rule](/articles/adages.html#pareto_pricipal)), is just render
twice, once as the depth pre-pass and then once again for shading, with the
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
