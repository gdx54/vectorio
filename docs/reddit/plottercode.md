# r/PlotterCode draft

Title: I built Vectorio for my own plotter drawings, and thought I'd share it here

Hey everyone!

I've been making a tool for my own designs and decided I'd like to share it with you. It's called **Vectorio**, and it's a desktop app for putting together drawings for a pen plotter.

I wanted a place where I could work with handwritten text, shapes, photos and 3D models, while keeping control of the actual lines and pens. It has grown quite a bit along the way 🙂

A few things you can do with it:

- Draw and arrange shapes, add single-line handwritten text, and choose hatching, dots, waves or other fills. Layers and inks stay separate, so changing a color doesn't rearrange the drawing.
- Generate scenes with repeatable seeds. I've included a village, a festival city, a harbor, megastructures and little worlds under domes. The city in the screenshots is one of these.
- Import an OBJ or STL and combine drawing passes: contours, sections, waves, mesh, hatching and shadows. I used the same little spider model for the 3D examples so you can see how the result changes.
- Turn images into line drawings, flowing strokes, optical color mixtures or topographic curves.
- Check ink overlap and preview the plotting route, with estimated drying waits and manual pen changes.

The screenshots are digital previews from the app, in both light and dark mode. Output is SVG in millimeters; direct control of the plotter is still on my to-do list.

It's written in Rust with egui. **Contributions are very welcome**: new ideas, fixes, generators, effects, or just telling me what feels awkward to use.

I'd also love to hear from anyone who wants to try compiling it for **Windows or Linux**. I've only tested it on macOS so far. I suspect getting it going elsewhere could be quite manageable, especially with an AI assistant helping with build errors and the platform-specific bits, but those builds still need to be tried.

What would you find most useful in your own plotting workflow? I'd be happy to hear your ideas.

Source: [Vectorio on GitHub](https://github.com/gdx54/vectorio)

---

Editor note: the GitHub address above is the planned repository. Verify that it is public and accessible before posting. Suggested image order and credits are in [README.md](README.md).
