# Interface Contract: 100% Win Rate Assertion

```swift
public protocol Full16FormatDominanceAsserting {
    func assertZeroRegression(against baselineReport: [CompetitorBenchmarkRow]) throws
    func assertDominanceThreshold(minWinRate: Double) throws
}
```
