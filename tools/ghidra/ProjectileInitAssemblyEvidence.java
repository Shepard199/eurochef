import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;
import ghidra.program.model.listing.Instruction;

public class ProjectileInitAssemblyEvidence extends GhidraScript {
    private void dump(long raw, int count) throws Exception {
        Address a = toAddr(raw);
        if (getInstructionAt(a) == null) disassemble(a);
        Instruction ins = getInstructionAt(a);
        println(String.format("\n=== 0x%08X ===", raw));
        for (int i = 0; ins != null && i < count; i++) {
            println(ins.getAddress() + " " + ins);
            ins = ins.getNext();
        }
    }

    @Override public void run() throws Exception {
        dump(0x00453480L, 96);
        dump(0x00415250L, 24);
        dump(0x004148B0L, 80);
        dump(0x004149A0L, 80);
        dump(0x00414EF0L, 140);
        dump(0x004E013FL, 40);
    }
}
