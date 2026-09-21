import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;
import ghidra.program.model.listing.Instruction;

public class MineProjectileSlotAssembly extends GhidraScript {
    private void dump(long raw) throws Exception {
        Address a=toAddr(raw); println(String.format("\n=== 0x%08X ===",raw));
        if(getInstructionAt(a)==null) disassemble(a);
        Instruction ins=getInstructionAt(a);
        for(int i=0;ins!=null&&i<140;i++){
            println(ins.getAddress()+" "+ins);
            if(ins.getMnemonicString().equalsIgnoreCase("RET")&&i>3) break;
            ins=ins.getNext();
        }
    }
    @Override public void run() throws Exception {
        dump(0x00415250L); dump(0x004148B0L); dump(0x004149A0L); dump(0x00416380L);
    }
}
