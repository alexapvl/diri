import XCTest
@testable import DiriPhone

/// The voice endpoints and the event framing, against fixtures of the
/// `diri-web` contract.
final class VoiceClientTests: XCTestCase {
    private var client: DiriClient!

    override func setUp() {
        super.setUp()
        VoiceStub.reset()
        let configuration = URLSessionConfiguration.ephemeral
        configuration.protocolClasses = [VoiceStub.self]
        client = DiriClient(
            endpoint: .init(baseURL: URL(string: "http://forge:7380")!, token: "tok"),
            configuration: configuration
        )
    }

    func testTranscriptCarriesLimitAndDecodesTurns() async throws {
        VoiceStub.json = #"{"turns":[{"role":"user","text":"hi","turn":1},{"role":"agent","text":"hello","turn":null}]}"#
        let turns = try await client.transcript(sessionID: "s_1", limit: 4)
        XCTAssertEqual(turns.map(\.role), ["user", "agent"])
        XCTAssertNil(turns[1].turn)
        let url = try XCTUnwrap(VoiceStub.lastRequest?.url)
        XCTAssertEqual(url.path, "/api/session/s_1/transcript")
        XCTAssertEqual(url.query, "limit=4")
        XCTAssertEqual(VoiceStub.lastRequest?.value(forHTTPHeaderField: "X-Diri-Token"), "tok")
    }

    func testSpokenToleratesNulls() async throws {
        VoiceStub.json = #"{"turn":null,"text":null}"#
        let spoken = try await client.spoken(sessionID: "s_1")
        XCTAssertEqual(spoken, .init(turn: nil, text: nil))
        XCTAssertEqual(VoiceStub.lastRequest?.url?.path, "/api/session/s_1/spoken")
    }

    func testReadAloudPostsTheFlag() async throws {
        VoiceStub.json = #"{"readAloud":true}"#
        try await client.setReadAloud(true)
        XCTAssertEqual(VoiceStub.lastRequest?.httpMethod, "POST")
        XCTAssertEqual(VoiceStub.lastRequest?.url?.path, "/api/voice")
        let body = try XCTUnwrap(VoiceStub.lastBody)
        let json = try XCTUnwrap(try JSONSerialization.jsonObject(with: body) as? [String: Any])
        XCTAssertEqual(json["readAloud"] as? Bool, true)
        let current = try await client.readAloud()
        XCTAssertTrue(current)
    }

    func testSSEParserFramesEventsAndDecodesTurnFinished() {
        var parser = SSEParser()
        var events: [DiriClient.ServerEvent] = []
        for line in ["event: session.updated", #"data: {"id":"s"}"#, "",
                     "event: turn_finished", #"data: {"sessionID":"s_1","turn":3,"spoken":"Done."}"#, "",
                     "event:ping", ""] {
            events += parser.feed(line)
        }
        XCTAssertEqual(events.map(\.name), ["session.updated", "turn_finished", "ping"])
        XCTAssertEqual(DiriClient.TurnFinished(events[1]), .init(sessionID: "s_1", turn: 3, spoken: "Done."))
        XCTAssertNil(DiriClient.TurnFinished(events[0]))
        let nulls = DiriClient.ServerEvent(name: "turn_finished", data: #"{"sessionID":"s","turn":null,"spoken":null}"#)
        XCTAssertEqual(DiriClient.TurnFinished(nulls), .init(sessionID: "s", turn: nil, spoken: nil))
    }

    func testUnknownNeedsInputKindStillDecodesTheSession() throws {
        let json = """
        {"id":"s_1","kind":{"claudeCode":{}},"cwd":"/r","projectID":"p","title":"t",
         "status":{"needsInput":{"_0":"plan-approval"}},
         "needsInput":{"kind":"plan-approval","riskHint":"exotic"},
         "pinned":false,"createdAt":1,"updatedAt":1}
        """
        let session = try JSONDecoder().decode(SessionRecord.self, from: Data(json.utf8))
        XCTAssertEqual(session.status, .needsInput(.other("plan-approval")))
        XCTAssertEqual(session.needsInput?.riskHint, .neutral)
        XCTAssertEqual(session.attention, .needsInput)
    }
}

private final class VoiceStub: URLProtocol {
    nonisolated(unsafe) static var json = "{}"
    nonisolated(unsafe) static var lastRequest: URLRequest?
    nonisolated(unsafe) static var lastBody: Data?

    static func reset() {
        json = "{}"
        lastRequest = nil
        lastBody = nil
    }

    override class func canInit(with request: URLRequest) -> Bool { true }
    override class func canonicalRequest(for request: URLRequest) -> URLRequest { request }

    override func startLoading() {
        Self.lastRequest = request
        Self.lastBody = request.httpBody ?? request.httpBodyStream.map { stream in
            stream.open()
            defer { stream.close() }
            var data = Data()
            var buffer = [UInt8](repeating: 0, count: 4096)
            while stream.hasBytesAvailable {
                let count = stream.read(&buffer, maxLength: buffer.count)
                if count <= 0 { break }
                data.append(buffer, count: count)
            }
            return data
        }
        let response = HTTPURLResponse(url: request.url!, statusCode: 200, httpVersion: nil,
                                       headerFields: ["Content-Type": "application/json"])!
        client?.urlProtocol(self, didReceive: response, cacheStoragePolicy: .notAllowed)
        client?.urlProtocol(self, didLoad: Data(Self.json.utf8))
        client?.urlProtocolDidFinishLoading(self)
    }

    override func stopLoading() {}
}
