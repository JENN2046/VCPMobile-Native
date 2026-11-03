import Foundation

/// Read-only server execution state previews. Never display IDs, request bodies,
/// extensions or infer that an original send / AA task has completed.
enum NativeExecutionLabels {
    static let maxJSONBytes = 131_072
    static let maxVisible = 24

    static func render(_ states: [String?], total: Int) -> [String] {
        precondition(total >= 0 && states.count <= maxVisible && states.count <= total)
        var output = states.map { raw -> String in
            guard let raw, !raw.isEmpty, raw.utf8.count <= 48,
                  raw.utf8.allSatisfy({ byte in
                      (65...90).contains(Int(byte)) || (97...122).contains(Int(byte)) ||
                      (48...57).contains(Int(byte)) || byte == 45 || byte == 95
                  }) else { return "服务端状态：未识别" }
            return "服务端状态：\(raw)"
        }
        if total > states.count { output.append("另有 \(total - states.count) 项状态未显示") }
        return output
    }

    /// Parse only the `state` string of each history page execution; never
    /// persist or log the full execution JSON. A malformed/oversized projection
    /// cannot become a false "completed" status or block history reception.
    static func project(_ json: String) -> [String] {
        guard let data = json.data(using: .utf8), data.count <= maxJSONBytes else {
            return ["任务状态快照过大，请在服务端核对"]
        }
        guard let records = (try? JSONSerialization.jsonObject(with: data)) as? [[String: Any]] else {
            return ["任务状态快照无效，请在服务端核对"]
        }
        let selected = records.prefix(maxVisible).map { $0["state"] as? String }
        return render(selected, total: records.count)
    }
}
