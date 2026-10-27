use super::GlassSurface;

pub fn find_non_overlapping_layers(surfaces: &[GlassSurface]) -> Vec<Vec<usize>> {
    let mut layers: Vec<Vec<usize>> = Vec::new();
    let mut current_layer: Vec<usize> = Vec::new();

    for (i, surface) in surfaces.iter().enumerate() {
        let [x, y, w, h] = surface.bounds;
        let mut overlaps_current = false;

        for &j in current_layer.iter() {
            let [ox, oy, ow, oh] = surfaces[j].bounds;
            if x < ox + ow && x + w > ox && y < oy + oh && y + h > oy {
                overlaps_current = true;
                break;
            }
        }

        if overlaps_current {
            layers.push(current_layer);
            current_layer = vec![i];
        } else {
            current_layer.push(i);
        }
    }

    if !current_layer.is_empty() {
        layers.push(current_layer);
    }

    layers
}
