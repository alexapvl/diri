import XCTest
@testable import DiriPhone

final class GatewayTests: XCTestCase {
    private final class MemoryStore: SecretStore, @unchecked Sendable {
        var items: [String: Data] = [:]
        var failWrites = false

        func read(_ account: String) -> Data? { items[account] }

        func write(_ account: String, _ data: Data?) -> Bool {
            if failWrites, data != nil { return false }
            items[account] = data
            return true
        }
    }

    private func endpoint(_ host: String, token: String = "t") -> DiriClient.Endpoint {
        .init(baseURL: URL(string: "http://\(host):7380")!, token: token)
    }

    // MARK: Keychain migration

    func testLegacySingleSlotMigratesToTheList() throws {
        let store = MemoryStore()
        store.items[GatewayStore.legacyAccount] = try JSONEncoder().encode(endpoint("100.90.0.2", token: "old"))

        let list = GatewayStore.load(from: store)

        XCTAssertEqual(list.count, 1)
        XCTAssertEqual(list[0].endpoint.token, "old")
        XCTAssertEqual(list[0].name, "100.90.0.2")
        XCTAssertNil(store.items[GatewayStore.legacyAccount], "legacy slot removed once migrated")
        // The migrated list is what the next launch reads.
        XCTAssertEqual(GatewayStore.load(from: store), list)
    }

    func testFailedMigrationWriteKeepsTheLegacySlot() throws {
        let store = MemoryStore()
        store.failWrites = true
        store.items[GatewayStore.legacyAccount] = try JSONEncoder().encode(endpoint("mac"))

        XCTAssertEqual(GatewayStore.load(from: store).count, 1)
        XCTAssertNotNil(store.items[GatewayStore.legacyAccount], "a failed write must not unpair the phone")
    }

    func testListWinsOverLegacyAndEmptyStoreIsEmpty() throws {
        let store = MemoryStore()
        XCTAssertEqual(GatewayStore.load(from: store), [])
        let saved = [Gateway(name: "Mac", endpoint: endpoint("mac")), Gateway(name: "forge", endpoint: endpoint("forge"))]
        XCTAssertTrue(GatewayStore.save(saved, to: store))
        store.items[GatewayStore.legacyAccount] = try JSONEncoder().encode(endpoint("stale"))
        XCTAssertEqual(GatewayStore.load(from: store), saved)
        XCTAssertTrue(GatewayStore.save([], to: store))
        XCTAssertNil(store.items[GatewayStore.listAccount])
    }

    func testRepairingTheSameAddressKeepsPriority() {
        let mac = Gateway(name: "Mac", endpoint: endpoint("mac", token: "a"))
        let forge = Gateway(name: "forge", endpoint: endpoint("forge"))
        let list = GatewayStore.adding(Gateway(name: "Mac", endpoint: endpoint("mac", token: "b")), to: [mac, forge])
        XCTAssertEqual(list.map(\.id), [mac.id, forge.id])
        XCTAssertEqual(list[0].endpoint.token, "b")
        XCTAssertEqual(GatewayStore.adding(Gateway(name: "pi", endpoint: endpoint("pi")), to: list).map(\.name),
                       ["Mac", "forge", "pi"])
    }

    // MARK: Selection + failover

    func testHighestPriorityReachableIsLive() {
        let mac = Gateway(name: "Mac", endpoint: endpoint("mac"))
        let forge = Gateway(name: "forge", endpoint: endpoint("forge"))
        let list = [mac, forge]
        XCTAssertEqual(GatewayProbe.live(list, outcomes: [mac.id: .reachable(host: "Mac"), forge.id: .reachable(host: "forge")]), mac)
        XCTAssertEqual(GatewayProbe.live(list, outcomes: [mac.id: .unreachable("down"), forge.id: .reachable(host: "forge")]), forge)
        XCTAssertEqual(GatewayProbe.live(list, outcomes: [mac.id: .unauthorized, forge.id: .reachable(host: "forge")]), forge)
        XCTAssertNil(GatewayProbe.live(list, outcomes: [mac.id: .unreachable("x")]))
    }

    func testProbesRunConcurrentlyAndASilentHostTimesOut() async {
        let mac = Gateway(name: "Mac", endpoint: endpoint("mac"))
        let forge = Gateway(name: "forge", endpoint: endpoint("forge"))
        let started = ContinuousClock.now
        let outcomes = await GatewayProbe.probeAll([mac, forge], timeout: .milliseconds(300)) { gateway in
            if gateway.name == "Mac" {
                // A Mac asleep with its lid closed: never answers.
                try? await Task.sleep(for: .seconds(30))
                return .reachable(host: "Mac")
            }
            try? await Task.sleep(for: .milliseconds(50))
            return .reachable(host: "forge")
        }
        let elapsed = ContinuousClock.now - started
        XCTAssertEqual(outcomes[forge.id], .reachable(host: "forge"))
        guard case .unreachable = outcomes[mac.id] else { return XCTFail("silent Mac must time out") }
        XCTAssertLessThan(elapsed, .seconds(2), "one silent host must not stall the probe")
        XCTAssertEqual(GatewayProbe.live([mac, forge], outcomes: outcomes), forge)
    }

    func testFailsBackWhenThePreferredHostReturns() async {
        let mac = Gateway(name: "Mac", endpoint: endpoint("mac"))
        let forge = Gateway(name: "forge", endpoint: endpoint("forge"))
        let up = await GatewayProbe.probeAll([mac, forge], timeout: .seconds(1)) { .reachable(host: $0.name) }
        XCTAssertEqual(GatewayProbe.live([mac, forge], outcomes: up), mac)
    }

    // MARK: Merged list

    @MainActor
    func testMergeTagsByGatewayOnlyWhenSeveralContribute() {
        func record(_ id: String) -> SessionRecord {
            SessionRecord(id: id, kind: .claudeCode, foregroundAgent: nil, cwd: "/r", projectID: "p",
                          worktreePath: nil, gitBranch: nil, title: id, status: .idle, needsInput: nil,
                          parent: nil, createdAt: 1, updatedAt: 1, lastSeenAt: nil, pinned: false,
                          archivedAt: nil, host: nil, hibernation: nil)
        }
        let mac = Gateway(name: "Mac", endpoint: endpoint("mac"))
        let forge = Gateway(name: "forge", endpoint: endpoint("forge"))
        let macList = DiriClient.SessionsResponse(host: "Mac", sessions: [record("a"), record("dup")], projects: nil)
        let forgeList = DiriClient.SessionsResponse(host: "forge", sessions: [record("b"), record("dup")], projects: nil)

        let single = AppModel.merge([(mac, macList)])
        XCTAssertEqual(single.sessions.map(\.gateway), [nil, nil])

        let merged = AppModel.merge([(mac, macList), (forge, forgeList)])
        XCTAssertEqual(merged.sessions.map(\.id), ["a", "dup", "b"])
        XCTAssertEqual(merged.sessions.map(\.gateway), ["Mac", "Mac", "forge"])
        XCTAssertEqual(merged.owner["b"], forge.id)
        XCTAssertEqual(merged.owner["dup"], mac.id, "a duplicate id belongs to the higher-priority gateway")
    }
}
