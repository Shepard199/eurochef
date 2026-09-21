import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;
import ghidra.program.model.listing.Instruction;
import ghidra.program.model.symbol.Reference;
import ghidra.program.model.listing.Function;

public class MineBotAttackPhaseEvidence extends GhidraScript {
    private void range(long start, long end) throws Exception {
        println(String.format("\n=== RANGE 0x%08X..0x%08X ===", start, end));
        Address a = toAddr(start);
        if (getInstructionAt(a) == null) disassemble(a);
        Instruction ins = getInstructionAt(a);
        while (ins != null && ins.getAddress().getOffset() < end) {
            println(ins.getAddress() + " " + ins);
            ins = ins.getNext();
        }
    }
    private void refs(long target) throws Exception {
        Address a = toAddr(target);
        println(String.format("\n=== REFS TO 0x%08X ===", target));
        for (Reference r : getReferencesTo(a)) {
            Function f = getFunctionContaining(r.getFromAddress());
            println(r.getFromAddress() + " " + r.getReferenceType() + " " +
                (f == null ? "NOFUNC" : f.getName() + "@" + f.getEntryPoint()));
        }
    }
    @Override public void run() throws Exception {
        range(0x00450C60L, 0x00450D40L);
        refs(0x00455DD0L);
        refs(0x00450C60L);
    }
}
