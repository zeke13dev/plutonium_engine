I've analyzed the current draw_halo and draw_rect implementations. draw_halo appears to be a radial/center-based glow, which
  is why it feels "radial/overlay" rather than following the perimeter. draw_rect only supports sharp borders.


  To achieve a true "neon" or "perimeter glow" that follows the rectangle's shape and corner radius, I propose the following
  engine spec for you to implement:

  Proposed Spec: draw_rect_glow

  This feature should render a glow that follows the signed distance field (SDF) of a rounded rectangle, rather than a simple
  circle.

  Method Signature:


   1 engine.draw_rect_glow(
   2     rect: Rectangle,
   3     color: [f32; 4],
   4     thickness: f32,    // Width of the core "sharp" line (0.0 for pure soft glow)
   5     glow_radius: f32,  // How far the soft glow extends from the edge
   6     corner_radius: f32, // Matches the underlying card's rounding
   7     intensity: f32,    // Alpha/brightness multiplier
   8     z: i32,
   9 )


  Shader Logic (SDF-based):
   1. SDF Calculation: Use a standard rounded-box SDF: dist = sdRoundedBox(pixel_pos - rect_center, rect_half_size,
      corner_radius).
   2. Core Line: If abs(dist) < thickness / 2.0, render the solid color.
   3. Glow Falloff: If dist > thickness / 2.0, the alpha should follow a curve like exp(-dist / glow_radius) or a smoothstep
      falloff up to glow_radius.
   4. Inner/Outer: The glow should ideally extend both slightly inward and outward from the perimeter for that "neon" look.


  Why this is better:
   * It will perfectly wrap around the corners of the gem cards.
   * It won't wash out the center of the gem (unlike the current radial halo).
   * It allows for a very tight "perimeter only" effect by keeping glow_radius small and thickness around 1.0–2.0.


  Does this spec cover what you need for the implementation, or should I refine the parameter set?
