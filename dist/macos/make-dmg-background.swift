// Generates the background images of the disk image (dmg-background.png and dmg-background@2x.png
// next to this script, run with `swift make-dmg-background.swift`). The window of the disk image is
// 540x380 points, the app is at (140, 170) and the Applications link at (400, 170) (see
// dmg-settings.py).

import AppKit

let scriptDirectory = URL(fileURLWithPath: CommandLine.arguments[0]).deletingLastPathComponent()

func render(scale: Int, to name: String) {
    let width = 540 * scale
    let height = 380 * scale
    let bitmap = NSBitmapImageRep(
        bitmapDataPlanes: nil, pixelsWide: width, pixelsHigh: height, bitsPerSample: 8,
        samplesPerPixel: 4, hasAlpha: true, isPlanar: false, colorSpaceName: .deviceRGB,
        bytesPerRow: 0, bitsPerPixel: 0)!
    // This makes the image 540x380 points (and sets its resolution), also for 2x. Drawing is in
    // points, the context scales to pixels by itself.
    bitmap.size = NSSize(width: 540, height: 380)
    NSGraphicsContext.saveGraphicsState()
    NSGraphicsContext.current = NSGraphicsContext(bitmapImageRep: bitmap)

    NSColor(calibratedWhite: 0.96, alpha: 1).setFill()
    NSRect(x: 0, y: 0, width: 540, height: 380).fill()

    // The icons are centered at y = 170 from the top, i.e. 210 from the bottom.
    let color = NSColor(calibratedWhite: 0.55, alpha: 1)
    color.setStroke()
    color.setFill()
    let arrow = NSBezierPath()
    arrow.lineWidth = 6
    arrow.lineCapStyle = .round
    arrow.move(to: NSPoint(x: 232, y: 210))
    arrow.line(to: NSPoint(x: 296, y: 210))
    arrow.stroke()
    let head = NSBezierPath()
    head.move(to: NSPoint(x: 290, y: 228))
    head.line(to: NSPoint(x: 316, y: 210))
    head.line(to: NSPoint(x: 290, y: 192))
    head.close()
    head.fill()

    NSGraphicsContext.restoreGraphicsState()
    let data = bitmap.representation(using: .png, properties: [:])!
    try! data.write(to: scriptDirectory.appendingPathComponent(name))
}

render(scale: 1, to: "dmg-background.png")
render(scale: 2, to: "dmg-background@2x.png")
