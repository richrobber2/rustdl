package app.rustdl;

public class DownloadNetworkPolicyTest {
    private static void check(int expected, boolean connected, boolean unmetered, String policy, boolean approved) {
        if (DownloadNetworkPolicy.decide(connected, unmetered, policy, approved) != expected) throw new AssertionError(policy);
    }
    public static void main(String[] args) {
        for (String policy : new String[]{"allow", "ask", "block", "invalid"}) {
            check(1, false, false, policy, false);
            check(1, false, true, policy, true);
            check(0, true, true, policy, false);
        }
        check(0, true, false, "allow", false);
        check(3, true, false, "ask", false);
        check(0, true, false, "ask", true);
        check(2, true, false, "block", false);
        check(2, true, false, "block", true);
        check(3, true, false, "invalid", false);
        System.out.println("Download network policy: offline, unmetered, allow, ask, block and approval precedence passed");
    }
}
