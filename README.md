# bevy_outline

Outline rendering based on the blurred buffer technique described in <https://alexanderameye.github.io/notes/rendering-outlines/>

<https://user-images.githubusercontent.com/8348954/209417581-da6c88cd-1155-4745-89b3-6584a0a5c29d.mp4>

## Getting Started

1. Add the `BlurredOutlinePlugin`
2. Add the `Outline` component to any mesh you want
3. Optionally, add the `OutlineSettings` to the camera to control the size of the outline. The size is only controllable per view because otherwise it would be too ineficient.

## TODO

Read this: <https://maxammann.org/posts/2022/01/wgpu-stencil-testing/>
