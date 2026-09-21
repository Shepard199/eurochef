import ghidra.app.decompiler.DecompInterface;
import ghidra.app.decompiler.DecompileResults;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;
import ghidra.program.model.listing.Function;

public class DogBotBehaviorEvidence extends GhidraScript {
    private void dump(long raw, DecompInterface decompiler) throws Exception {
        Address address = toAddr(raw);
        Function function = getFunctionContaining(address);
        println(String.format("\n=== TARGET 0x%08X ===", raw));
        if (function == null) {
            println("NO_FUNCTION");
            return;
        }
        println("FUNCTION=" + function.getName() + " ENTRY=" + function.getEntryPoint());
        DecompileResults results = decompiler.decompileFunction(function, 60, monitor);
        if (results.decompileCompleted() && results.getDecompiledFunction() != null) {
            println(results.getDecompiledFunction().getC());
        }
    }

    @Override public void run() throws Exception {
        DecompInterface d = new DecompInterface();
        d.openProgram(currentProgram);
        long[] targets = new long[] {
            0x004514F0L, 0x004594F0L, 0x00460870L, 0x00456CB0L, 0x00434CF0L, 0x00456E20L, 0x0045B190L,
            0x0046BC20L, 0x0046D9E0L, 0x0046A850L, 0x0046CB30L,
            0x0044EE20L, 0x00457980L, 0x00457E30L, 0x004581C0L,
            0x004583A0L, 0x004584C0L, 0x004590A0L, 0x00469E20L
        };
        for (long target : targets) dump(target, d);
        d.dispose();
    }
}
