import ghidra.app.decompiler.DecompInterface;
import ghidra.app.decompiler.DecompileResults;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;
import ghidra.program.model.listing.Function;
import ghidra.program.model.listing.Instruction;

public class CommonProjectileCreateEvidence extends GhidraScript {
    private void dump(long raw, DecompInterface d) throws Exception {
        Address a=toAddr(raw); println(String.format("\n=== 0x%08X ===",raw));
        if(getInstructionAt(a)==null) disassemble(a);
        Instruction ins=getInstructionAt(a);
        for(int i=0;ins!=null&&i<320;i++){
            println(ins.getAddress()+" "+ins);
            ins=ins.getNext();
        }
        Function f=getFunctionContaining(a);
        if(f!=null){
            println("FUNCTION="+f.getName()+" ENTRY="+f.getEntryPoint());
            DecompileResults r=d.decompileFunction(f,60,monitor);
            if(r.decompileCompleted()&&r.getDecompiledFunction()!=null){println(r.getDecompiledFunction().getC());}
        }
    }
    @Override public void run() throws Exception {
        DecompInterface d=new DecompInterface(); d.openProgram(currentProgram);
        dump(0x0044F1CCL,d);
        dump(0x004DFEB0L,d);
        d.dispose();
    }
}
