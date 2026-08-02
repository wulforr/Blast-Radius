const which = Math.random() > 0.5 ? './a' : './b'
export const load = () => import(which)
export const known = () => import('./c')
