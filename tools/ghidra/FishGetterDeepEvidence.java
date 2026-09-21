import ghidra.app.decompiler.DecompInterface;
import ghidra.app.decompiler.DecompileResults;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;
import ghidra.program.model.listing.Function;
import ghidra.program.model.listing.InstructionIterator;

public class FishGetterDeepEvidence extends GhidraScript {
    private Function ensure(long raw)throws Exception{
        Address a=toAddr(raw); disassemble(a);
        Function f=getFunctionAt(a);
        if(f==null){try{f=createFunction(a,null);}catch(Exception e){println("CREATE "+Long.toHexString(raw)+" "+e.getMessage());}}
        if(f==null)f=getFunctionContaining(a);return f;
    }
    private void dump(long raw,DecompInterface d)throws Exception{
        Function f=ensure(raw);
        println(String.format("\n=== 0x%08X %s entry=%s ===",raw,f==null?"<missing>":f.getName(),f==null?"":f.getEntryPoint()));
        InstructionIterator it=currentProgram.getListing().getInstructions(toAddr(raw),true);
        int n=0; while(it.hasNext()&&n<40){var ins=it.next();println(ins.getAddress()+" "+ins);n++;}
        if(f!=null){DecompileResults r=d.decompileFunction(f,60,monitor);if(r.decompileCompleted()&&r.getDecompiledFunction()!=null)println(r.getDecompiledFunction().getC());}
    }
    @Override public void run()throws Exception{
        DecompInterface d=new DecompInterface();d.openProgram(currentProgram);
        dump(0x004800C0L,d);dump(0x004800E0L,d);dump(0x00480130L,d);dump(0x004803D0L,d);
        d.dispose();
    }
}
