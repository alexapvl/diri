import Foundation

/// The on-device scrollback: history lines by absolute index, kept in fixed
/// pages so the network is asked for whole pages and memory is given back a
/// page at a time.
///
/// Scrolling reads only from here. A line that is not cached draws as blank
/// and its page is requested; nothing on the scroll path ever waits for a
/// reply, which is what keeps a remote session's scrollback as fast as a local
/// one.
struct LineCache {
    static let pageSize = 256

    /// Lines kept at most, rounded to pages. 40k coloured lines is ~10 MB.
    let capacity: Int

    private var pages: [Int: Page] = [:]
    private var clock: UInt64 = 0
    /// Pages requested and not yet answered, so a fast scroll asks once.
    private(set) var inFlight: Set<Int> = []

    private struct Page {
        var lines: [TermLine?]
        var filled: Int
        var used: UInt64
    }

    init(capacity: Int = 40_000) {
        self.capacity = max(capacity, Self.pageSize)
    }

    var pageCount: Int { pages.count }
    var lineCount: Int { pages.values.reduce(0) { $0 + $1.filled } }

    static func page(of index: Int) -> Int { index >= 0 ? index / pageSize : -1 }

    /// Looks a line up and marks its page as recently used.
    mutating func line(at index: Int) -> TermLine? {
        let number = Self.page(of: index)
        guard number >= 0, pages[number] != nil else { return nil }
        clock += 1
        pages[number]!.used = clock
        return pages[number]!.lines[index - number * Self.pageSize]
    }

    /// Looks a line up without touching recency (for tests and peeking).
    func peek(_ index: Int) -> TermLine? {
        let number = Self.page(of: index)
        guard number >= 0, let page = pages[number] else { return nil }
        return page.lines[index - number * Self.pageSize]
    }

    /// Stores `lines` starting at absolute index `start`. `protecting` pages
    /// (the ones on screen) survive the eviction this may trigger.
    mutating func insert(_ lines: [TermLine], at start: Int, protecting: ClosedRange<Int>? = nil) {
        guard !lines.isEmpty else { return }
        clock += 1
        for (offset, line) in lines.enumerated() {
            let index = start + offset
            guard index >= 0 else { continue }
            let number = Self.page(of: index)
            if pages[number] == nil {
                pages[number] = Page(lines: Array(repeating: nil, count: Self.pageSize), filled: 0, used: clock)
            }
            let slot = index - number * Self.pageSize
            // Mutated in place through the dictionary: copying the page out
            // and back would copy its whole line array once per line.
            if pages[number]!.lines[slot] == nil { pages[number]!.filled += 1 }
            pages[number]!.lines[slot] = line
            pages[number]!.used = clock
        }
        evict(protecting: protecting)
    }

    /// Pages in `range` (absolute line indices, clamped to `historySize`) that
    /// have a hole and are not already requested — nearest to `focus` first.
    func missingPages(in range: Range<Int>, historySize: Int, focus: Int) -> [Int] {
        let lower = max(floor, range.lowerBound)
        let upper = min(historySize, range.upperBound)
        guard lower < upper else { return [] }
        var result: [Int] = []
        for number in Self.page(of: lower) ... Self.page(of: upper - 1) where !inFlight.contains(number) {
            if !isComplete(number, historySize: historySize) { result.append(number) }
        }
        let focusPage = Self.page(of: max(0, focus))
        return result.sorted { abs($0 - focusPage) < abs($1 - focusPage) }
    }

    func isComplete(_ number: Int, historySize: Int) -> Bool {
        let start = number * Self.pageSize
        let first = max(0, floor - start)
        let needed = min(Self.pageSize, historySize - start)
        guard needed > first else { return true }
        guard let page = pages[number] else { return false }
        if page.filled >= needed - first { return (first ..< needed).allSatisfy { page.lines[$0] != nil } }
        return false
    }

    /// The oldest line the server still has. A bounded scrollback drops its
    /// oldest rows; indices below this are gone for good and never requested.
    private(set) var floor = 0

    mutating func setFloor(_ index: Int) { floor = max(floor, index) }

    mutating func markRequested(_ number: Int) { inFlight.insert(number) }
    mutating func markAnswered(_ number: Int) { inFlight.remove(number) }

    /// Forgets everything: the history was rewritten (a resize reflowed it, or
    /// the server trimmed its oldest lines).
    mutating func removeAll() {
        pages.removeAll()
        floor = 0
        inFlight.removeAll()
    }

    /// Drops lines at or after `index` (history shrank).
    mutating func truncate(from index: Int) {
        let first = Self.page(of: max(0, index))
        for number in pages.keys where number > first { pages.removeValue(forKey: number) }
        if var page = pages.removeValue(forKey: first) {
            for slot in (index - first * Self.pageSize) ..< Self.pageSize where page.lines[slot] != nil {
                page.lines[slot] = nil
                page.filled -= 1
            }
            pages[first] = page
        }
    }

    private mutating func evict(protecting: ClosedRange<Int>?) {
        let maxPages = max(1, capacity / Self.pageSize)
        guard pages.count > maxPages else { return }
        let victims = pages
            .filter { protecting?.contains($0.key) != true }
            .sorted { $0.value.used < $1.value.used }
            .prefix(pages.count - maxPages)
        for victim in victims { pages.removeValue(forKey: victim.key) }
    }
}
