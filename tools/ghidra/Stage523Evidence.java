import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;
import ghidra.program.model.listing.Instruction;
import ghidra.program.model.listing.InstructionIterator;
import ghidra.program.model.listing.Listing;
import ghidra.program.model.symbol.Reference;

public class Stage523Evidence extends GhidraScript {
    private void dumpLinear(long raw, int maxInstructions) throws Exception {
        Address start = toAddr(raw);
        Listing listing = currentProgram.getListing();
        disassemble(start);
        println(String.format("TARGET 0x%08X", raw));
        InstructionIterator it = listing.getInstructions(start, true);
        int count = 0;
        while (it.hasNext() && count < maxInstructions) {
            Instruction ins = it.next();
            println("  " + ins.getAddress() + "  " + ins.toString());
            for (Reference ref : ins.getReferencesFrom()) {
                println("    REF " + ref.getReferenceType() + " -> " + ref.getToAddress());
            }
            count++;
        }
        println("  instructions_dumped=" + count);
    }

    @Override
    public void run() throws Exception {
        println("PROGRAM=" + currentProgram.getName());
        dumpLinear(0x004CDF10L, 700);
        dumpLinear(0x004CE620L, 180);
        dumpLinear(0x004CE690L, 120);
        dumpLinear(0x00509C6EL, 80);
        dumpLinear(0x00509C48L, 80);
    }
}
