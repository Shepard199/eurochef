import ghidra.app.decompiler.DecompInterface;
import ghidra.app.decompiler.DecompileResults;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.listing.Function;

public class FishTriggerGetterEvidence extends GhidraScript {
    private long ptr(long a)throws Exception{return Integer.toUnsignedLong(getInt(toAddr(a)));}
    private void dump(long raw,DecompInterface d)throws Exception{
        Function f=getFunctionAt(toAddr(raw)); if(f==null)f=getFunctionContaining(toAddr(raw));
        println(String.format("\n=== 0x%08X %s ===",raw,f==null?"<missing>":f.getName()));
        if(f==null)return;DecompileResults r=d.decompileFunction(f,60,monitor);
        if(r.decompileCompleted()&&r.getDecompiledFunction()!=null)println(r.getDecompiledFunction().getC());
    }
    @Override public void run()throws Exception{
        long vt=0x005E83E0L;
        DecompInterface d=new DecompInterface();d.openProgram(currentProgram);
        for(int off=0x100;off<=0x120;off+=4){
            long f=ptr(vt+off);
            println(String.format("slot +0x%03X -> 0x%08X",off,f));
            if(f>=0x00400000L&&f<0x00590000L)dump(f,d);
        }
        d.dispose();
    }
}
