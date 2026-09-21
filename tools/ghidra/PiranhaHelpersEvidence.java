import ghidra.app.decompiler.DecompInterface;
import ghidra.app.decompiler.DecompileResults;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;
import ghidra.program.model.listing.Function;
import ghidra.program.model.symbol.Symbol;

public class PiranhaHelpersEvidence extends GhidraScript {
    private Function ensure(long raw) throws Exception {
        Address a = toAddr(raw);
        Function f = getFunctionAt(a);
        if (f == null) {
            disassemble(a);
            try { f = createFunction(a, null); } catch (Exception ignored) {}
        }
        if (f == null) f = getFunctionContaining(a);
        return f;
    }

    private void dump(long raw, DecompInterface d) throws Exception {
        Function f = ensure(raw);
        println(String.format("\n=== 0x%08X %s ===", raw, f == null ? "<missing>" : f.getName()));
        if (f == null) return;
        DecompileResults r = d.decompileFunction(f, 90, monitor);
        if (r.decompileCompleted() && r.getDecompiledFunction() != null) {
            println(r.getDecompiledFunction().getC());
        }
    }

    private void ptrInfo(long raw) throws Exception {
        long p = Integer.toUnsignedLong(getInt(toAddr(raw)));
        println(String.format("PTR 0x%08X -> 0x%08X", raw, p));
        Symbol s = getSymbolAt(toAddr(p));
        if (s != null) println("  symbol=" + s.getName());
    }

    @Override
    public void run() throws Exception {
        DecompInterface d = new DecompInterface();
        d.openProgram(currentProgram);
        dump(0x00467B50L, d);
        dump(0x00454CA0L, d);
        dump(0x0044CD10L, d);
        dump(0x00455E30L, d);
        ptrInfo(0x005EBE08L);
        ptrInfo(0x005DFDF4L);
        d.dispose();
    }
}
