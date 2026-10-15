import ghidra.app.decompiler.*;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.listing.*;
import ghidra.program.model.symbol.*;
import java.util.*;
public class LowLevelInputBackendEvidence extends GhidraScript {
 DecompInterface d=new DecompInterface();
 long u32(long a)throws Exception{byte[] b=new byte[4];currentProgram.getMemory().getBytes(toAddr(a),b);return ((long)b[0]&255)|(((long)b[1]&255)<<8)|(((long)b[2]&255)<<16)|(((long)b[3]&255)<<24);}
 void dec(long x)throws Exception{
   Function f=getFunctionAt(toAddr(x)); if(f==null)f=getFunctionContaining(toAddr(x));
   println(String.format("\n=== DEC 0x%08X ===",x));
   if(f==null){println("NO_FUNCTION");return;}
   println(f.getName()+" @ "+f.getEntryPoint());
   DecompileResults r=d.decompileFunction(f,90,monitor);
   if(r.decompileCompleted()&&r.getDecompiledFunction()!=null)println(r.getDecompiledFunction().getC());
 }
 public void run()throws Exception{
   d.openProgram(currentProgram);
   long base=0x005e07d0L;
   println("=== VTABLE 0x005E07D0 ===");
   for(int off=0;off<=0x60;off+=4){
     long p=u32(base+off);Function f=getFunctionAt(toAddr(p));
     println(String.format("+0x%02X 0x%08X %s",off,p,f==null?"":f.getName()));
   }
   dec(0x00534e6cL);
   for(int off=0;off<=0x30;off+=4){long p=u32(base+off); if(p>=0x00400000L&&p<0x00600000L)dec(p);}
   d.dispose();
 }
}