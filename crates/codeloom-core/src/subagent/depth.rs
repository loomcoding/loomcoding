//! Política de recursión de los subagentes: cuántos niveles le quedan a cada sesión y quién puede delegar.

/// Niveles restantes de la sesión raíz: ella misma más los del techo de profundidad.
pub fn root_levels(ceiling: u32) -> u32 {
    ceiling.saturating_add(1)
}

/// Niveles restantes de un hijo: los del padre menos uno, sin pasar de lo que su perfil declara (sin
/// `max_depth`, una hoja).
pub fn child_levels(parent_left: u32, profile_max_depth: Option<u32>) -> u32 {
    parent_left
        .saturating_sub(1)
        .min(profile_max_depth.unwrap_or(1).max(1))
}

/// Si el perfil pide más niveles de los que el techo del árbol deja al hijo.
pub fn depth_clipped(parent_left: u32, profile_max_depth: Option<u32>) -> bool {
    profile_max_depth.is_some_and(|pedidos| pedidos > parent_left.saturating_sub(1))
}

/// Una sesión puede delegar solo si le quedan al menos dos niveles: ella y uno por debajo.
pub fn can_delegate(left: u32) -> bool {
    left >= 2
}
