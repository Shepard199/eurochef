import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;
import ghidra.program.model.listing.Function;
import ghidra.program.model.symbol.Reference;

public class DogBotAttackClockEvidence extends GhidraScript {
    @Override public void run() throws Exception {
        long[] globals = {0x007B2A18L, 0x007B2A3CL};
        for (long raw : globals) {
            Address target = toAddr(raw);
            println(String.format("=== refs 0x%08X ===", raw));
            for (Reference ref : getReferencesTo(target)) {
                Function f = getFunctionContaining(ref.getFromAddress());
                println(ref.getFromAddress()+" "+ref.getReferenceType()+" owner="+
                    (f == null ? "NOFUNC" : f.getName()+"@"+f.getEntryPoint()));
            }
        }
    }
}
