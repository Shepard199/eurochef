import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;
import ghidra.program.model.listing.Data;
import ghidra.program.model.listing.Function;
import ghidra.program.model.mem.MemoryBlock;

public class MineBotSpeedOwnerEvidence extends GhidraScript {
    private void refs(long raw) throws Exception {
        Address target = toAddr(raw);
        println(String.format("\n=== REFS TO 0x%08X ===", raw));
        for (var ref : getReferencesTo(target)) {
            Address from = ref.getFromAddress();
            Function f = getFunctionContaining(from);
            println(from + " " + ref.getReferenceType() + " " + (f == null ? "NOFUNC" : f.getName()+"@"+f.getEntryPoint()));
        }
    }

    @Override
    public void run() throws Exception {
        refs(0x0045F910L);
        refs(0x00462330L);
        refs(0x00462C00L);
        refs(0x00462F90L);
        refs(0x00463BD0L);
        refs(0x00459D70L);
        refs(0x0045FEA0L);
    }
}
