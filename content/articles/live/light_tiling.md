---
title: Light Tiling, Live!
---
_Following along with Renderling's initial implementation of light tiling_

# Let's do some Light Tiling - Sat 22 Mar 

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

Also, Godot and Unity both use Forward+.
