import os
import QuartzCore
import UIKit

/// Frame-time probe for the terminal's scroll path.
///
/// A display link runs only while the terminal is moving and records, per
/// frame, the interval between presented frames and the CPU time the scroll
/// layout took. A frame counts as dropped when its interval exceeds one and a
/// half refresh periods. Results are logged with `os_signpost` intervals (for
/// Instruments) and summarised on the `frame-probe` accessibility element (for
/// UI tests and the benchmark log).
@MainActor
final class FrameProbe: NSObject {
    static let shared = FrameProbe()
    static let log = OSLog(subsystem: "com.cristicretu.diri.phone", category: "TerminalScroll")

    struct Report: CustomStringConvertible {
        var frames = 0
        var dropped = 0
        var refreshHz = 0.0
        var p50 = 0.0
        var p95 = 0.0
        var worst = 0.0
        var layoutP95 = 0.0
        var layoutWorst = 0.0
        var lines = 0
        /// Distinct history lines shown blank because their page had not arrived.
        var blank = 0

        var description: String {
            String(format: "frames=%d dropped=%d (%.2f%%) refresh=%.0fHz interval p50=%.2fms p95=%.2fms max=%.2fms layout p95=%.3fms max=%.3fms lines travelled=%d blank=%d",
                   frames, dropped, frames > 0 ? Double(dropped) * 100 / Double(frames) : 0, refreshHz,
                   p50, p95, worst, layoutP95, layoutWorst, lines, blank)
        }
    }

    private var link: CADisplayLink?
    private var last: CFTimeInterval = 0
    private var intervals: [Double] = []
    private var periods: [Double] = []
    private var layoutCosts: [Double] = []
    private var pendingLayout = 0.0
    private var idleFrames = 0
    private var signpost = OSSignpostID(log: FrameProbe.log)
    private(set) var lastReport = Report()
    private var lowest = Int.max
    private var highest = 0
    private var travelled = 0
    private var misses = Set<Int>()

    func noteMiss(_ index: Int) { if link != nil { misses.insert(index) } }
    private var lastFirst: Int?
    var onReport: ((Report) -> Void)?

    /// Called from the view while it is scrolling.
    func noteScrolling() {
        idleFrames = 0
        guard link == nil else { return }
        intervals.removeAll(keepingCapacity: true)
        periods.removeAll(keepingCapacity: true)
        layoutCosts.removeAll(keepingCapacity: true)
        lowest = .max
        highest = 0
        travelled = 0
        misses.removeAll()
        lastFirst = nil
        last = 0
        signpost = OSSignpostID(log: Self.log)
        os_signpost(.begin, log: Self.log, name: "Fling", signpostID: signpost)
        let link = CADisplayLink(target: self, selector: #selector(tick(_:)))
        link.preferredFrameRateRange = CAFrameRateRange(minimum: 60, maximum: 120, preferred: 120)
        link.add(to: .main, forMode: .common)
        self.link = link
    }

    func noteLayout(seconds: Double, firstLine: Int, lastLine: Int) {
        pendingLayout += seconds
        lowest = min(lowest, firstLine)
        highest = max(highest, lastLine)
        if let lastFirst { travelled += abs(firstLine - lastFirst) }
        lastFirst = firstLine
    }

    /// Line drawing happens in the Core Animation commit, after layout; it is
    /// part of the frame's main-thread cost too.
    func noteDraw(seconds: Double) {
        if link != nil { pendingLayout += seconds }
    }

    @objc private func tick(_ link: CADisplayLink) {
        let period = link.targetTimestamp - link.timestamp
        if last > 0 {
            intervals.append((link.timestamp - last) * 1000)
            periods.append(period * 1000)
            layoutCosts.append(pendingLayout * 1000)
        }
        pendingLayout = 0
        last = link.timestamp
        idleFrames += 1
        // A quarter second without a scroll step ends the measurement.
        if idleFrames > Int(0.25 / max(period, 1.0 / 120)) { finish() }
    }

    func finish() {
        guard let link else { return }
        link.invalidate()
        self.link = nil
        os_signpost(.end, log: Self.log, name: "Fling", signpostID: signpost)
        // Drop the idle tail that ended the measurement.
        let keep = max(0, intervals.count - idleFrames + 1)
        let frameIntervals = Array(intervals.prefix(keep))
        let framePeriods = Array(periods.prefix(keep))
        guard !frameIntervals.isEmpty else { return }
        var report = Report()
        report.frames = frameIntervals.count
        let period = framePeriods.sorted()[framePeriods.count / 2]
        report.refreshHz = period > 0 ? 1000 / period : 0
        report.dropped = frameIntervals.filter { $0 > period * 1.5 }.count
        let sorted = frameIntervals.sorted()
        report.p50 = sorted[sorted.count / 2]
        report.p95 = sorted[min(sorted.count - 1, Int(Double(sorted.count) * 0.95))]
        report.worst = sorted.last ?? 0
        let costs = layoutCosts.prefix(keep).sorted()
        report.layoutP95 = costs.isEmpty ? 0 : costs[min(costs.count - 1, Int(Double(costs.count) * 0.95))]
        report.layoutWorst = costs.last ?? 0
        report.lines = travelled
        report.blank = misses.count
        lastReport = report
        NSLog("DIRI_FRAME_PROBE %@", report.description)
        onReport?(report)
    }
}
