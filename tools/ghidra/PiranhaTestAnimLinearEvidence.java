import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;
import ghidra.program.model.listing.Instruction;
import ghidra.program.model.listing.InstructionIterator;
import ghidra.program.model.listing.Listing;

public class PiranhaTestAnimLinearEvidence extends GhidraScript {
    private void dump(long raw, int count) throws Exception {
        Address start = toAddr(raw);
        disassemble(start);
        Listing listing = currentProgram.getListing();
        InstructionIterator it = listing.getInstructions(start, true);
        println(String.format("\n=== LINEAR 0x%08X ===", raw));
        int n = 0;
        while (it.hasNext() && n++ < count) {
            Instruction ins = it.next();
            println(ins.getAddress() + "  " + ins.toString());
        }
    }
    @Override public void run() throws Exception {
        dump(0x0045ABE0L, 120);
        dump(0x0045AD00L, 120);
    }
}
